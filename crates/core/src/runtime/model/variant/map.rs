use std::borrow::Cow;

use super::*;

pub type VariantObjectMap = BTreeMap<String, Variant>;

pub trait VariantObject {
    fn contains_property(&self, prop: &str) -> bool;
    fn get_property(&self, prop: &str) -> Option<&Variant>;
    fn get_property_mut(&mut self, prop: &str) -> Option<&mut Variant>;
    fn get_nav_property(&self, self_name: &str, expr: &str) -> Option<&Variant>;
    fn get_nav_property_mut(&mut self, self_name: &str, expr: &str) -> Option<&mut Variant>;
    fn set_property(&mut self, prop: String, value: Variant);
    fn set_nav_property(
        &mut self,
        self_name: &str,
        expr: &str,
        value: Variant,
        create_missing: bool,
    ) -> crate::Result<()>;

    fn get_seg_property(&self, segs: &[PropexSegment]) -> Option<&Variant>;
    fn get_seg_property_mut(&mut self, segs: &[PropexSegment]) -> Option<&mut Variant>;
    fn evaluate_seg_property(&self, self_name: &str, segs: &mut [PropexSegment]) -> crate::Result<()>;

    fn remove_property(&mut self, prop: &str) -> Option<Variant>;
    fn remove_nav_property(&mut self, expr: &str) -> Option<Variant>;
    fn remove_seg_property(&mut self, segs: &[PropexSegment]) -> Option<Variant>;
}

impl VariantObject for VariantObjectMap {
    fn contains_property(&self, prop: &str) -> bool {
        self.contains_key(prop)
    }

    fn get_property(&self, prop: &str) -> Option<&Variant> {
        self.get(prop)
    }

    fn get_property_mut(&mut self, prop: &str) -> Option<&mut Variant> {
        self.get_mut(prop)
    }

    /// Get the value of a navigation property
    ///
    /// The first level of the property expression for 'msg' must be a string, which means it must be
    /// `msg[msg.topic]` `msg['aaa']` or `msg.aaa`, and not `msg[12]`
    fn get_nav_property(&self, self_name: &str, expr: &str) -> Option<&Variant> {
        let mut segs = propex::parse(expr).ok()?;
        self.evaluate_seg_property(self_name, &mut segs).ok()?;
        self.get_seg_property(&segs)
    }

    fn get_nav_property_mut(&mut self, self_name: &str, expr: &str) -> Option<&mut Variant> {
        let mut segs = propex::parse(expr).ok()?;
        self.evaluate_seg_property(self_name, &mut segs).ok()?;
        self.get_seg_property_mut(&segs)
    }

    /// Set the value of a direct property.
    fn set_property(&mut self, prop: String, value: Variant) {
        let _ = self.insert(prop, value);
    }

    /// Set the value of a navigation property.
    fn set_nav_property(
        &mut self,
        self_name: &str,
        expr: &str,
        value: Variant,
        create_missing: bool,
    ) -> crate::Result<()> {
        if expr.is_empty() {
            return Err(crate::EdgelinkError::BadArguments("The argument expr cannot be empty".to_string()).into());
        }

        let mut segs = propex::parse(expr).map_err(|e| crate::EdgelinkError::BadArguments(e.to_string()))?;
        self.evaluate_seg_property(self_name, &mut segs)?;

        let first_prop_name = match segs.first() {
            Some(PropexSegment::Property(name)) => name,
            _ => {
                return Err(crate::EdgelinkError::BadArguments(format!(
                    "The first property to access must be a string, but got '{}'",
                    expr
                ))
                .into())
            }
        };

        // If create_missing is true and first_prop doesn't exist, we should create it here.
        let first_prop = match (self.get_property_mut(first_prop_name), create_missing, segs.len()) {
            (Some(prop), _, _) => prop,
            (None, true, 1) => {
                // Only one level of the property
                self.insert(expr.into(), value);
                return Ok(());
            }
            (None, true, _) => {
                let next_seg = segs.get(1);
                let var = match next_seg {
                    // the next level property is an object
                    Some(PropexSegment::Property(_)) => Variant::empty_object(),
                    Some(PropexSegment::Index(_)) => Variant::empty_array(),
                    _ => {
                        return Err(crate::EdgelinkError::BadArguments(format!(
                            "Not allowed to set first property: '{}'",
                            first_prop_name
                        ))
                        .into());
                    }
                };
                self.insert(first_prop_name.to_string(), var);
                self.get_property_mut(first_prop_name).unwrap()
            }
            (None, _, _) => {
                return Err(crate::EdgelinkError::BadArguments(format!(
                    "Failed to set first property: '{}'",
                    first_prop_name
                ))
                .into());
            }
        };

        if segs.len() == 1 {
            *first_prop = value;
            return Ok(());
        }

        match first_prop.get_item_by_propex_segments_mut(&segs[1..]) {
            Some(pv) => {
                *pv = value;
                Ok(())
            }
            None if create_missing => {
                first_prop.set_property_by_propex_segments(&segs[1..], value, true).map_err(Into::into)
            }
            None => Err(crate::EdgelinkError::InvalidOperation(
                "Unable to set property: missing intermediate segments".into(),
            )
            .into()),
        }
    }

    fn get_seg_property(&self, segs: &[PropexSegment]) -> Option<&Variant> {
        match segs {
            [PropexSegment::Property(first_prop_name)] => self.get(first_prop_name.as_ref()),
            [PropexSegment::Property(first_prop_name), ref rest @ ..] => {
                self.get(first_prop_name.as_ref())?.get_item_by_propex_segments(rest)
            }
            _ => None,
        }
    }

    fn get_seg_property_mut(&mut self, segs: &[PropexSegment]) -> Option<&mut Variant> {
        match segs {
            [PropexSegment::Property(first_prop_name)] => self.get_property_mut(first_prop_name),
            [PropexSegment::Property(first_prop_name), ref rest @ ..] => {
                self.get_property_mut(first_prop_name)?.get_item_by_propex_segments_mut(rest)
            }
            _ => None,
        }
    }

    fn evaluate_seg_property(&self, self_name: &str, segs: &mut [PropexSegment]) -> crate::Result<()> {
        for seg in segs.iter_mut() {
            if let PropexSegment::Nested(nested_segs) = seg {
                if nested_segs.first() != Some(&PropexSegment::Property(Cow::Borrowed(self_name))) {
                    return Err(
                        EdgelinkError::BadArguments(format!("The expression must contains `{}.`", self_name)).into()
                    );
                }
                *seg = match self.get_seg_property(&nested_segs[1..]).ok_or(EdgelinkError::OutOfRange)? {
                    Variant::String(str_index) => PropexSegment::Property(Cow::Owned(str_index.clone())),
                    Variant::Integer(int_index) if *int_index >= 0 => PropexSegment::Index(*int_index as usize),
                    Variant::Rational(f64_index) if *f64_index >= 0.0 => {
                        PropexSegment::Index(f64_index.round() as usize)
                    }
                    _ => return Err(EdgelinkError::OutOfRange.into()), // We cannot found the nested property
                };
            }
        }
        Ok(())
    }

    fn remove_property(&mut self, prop: &str) -> Option<Variant> {
        self.remove(prop)
    }

    /// Remove the value of a navigation property.
    fn remove_nav_property(&mut self, expr: &str) -> Option<Variant> {
        // Return None if the expression is empty.
        if expr.is_empty() {
            return None;
        }

        // Parse the expression into segments.
        // TODO nested
        let segs = propex::parse(expr).ok()?;

        self.remove_seg_property(&segs)
    }

    fn remove_seg_property(&mut self, segs: &[PropexSegment]) -> Option<Variant> {
        // Return None if the expression is empty.
        if segs.is_empty() {
            return None;
        }
        // Handle the parsed segments.
        match segs {
            // If there's only one segment, remove the property directly.
            [PropexSegment::Property(first_prop_name)] => self.remove(first_prop_name.as_ref()),

            // If there are multiple segments, navigate through the nested structure.
            [PropexSegment::Property(first_prop_name), ref rest @ ..] => {
                // Get the mutable reference to the navigation property.
                let prop_tail =
                    self.get_mut(first_prop_name.as_ref())?.get_item_by_propex_segments_mut(&rest[..rest.len() - 1])?;

                // Remove the value based on the type of the last segment.
                match (prop_tail, segs.last()?) {
                    (Variant::Object(tail_map), PropexSegment::Property(tail_seg)) => {
                        tail_map.remove(tail_seg.as_ref())
                    }
                    (Variant::Array(tail_arr), PropexSegment::Index(tail_index)) => Some(tail_arr.remove(*tail_index)),
                    _ => None,
                }
            }

            // If the segments don't match the expected pattern, return None.
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_nav_property() {
        let mut obj1 = Variant::from([
            ("value1", Variant::Integer(123)),
            ("value2", Variant::Rational(123.0)),
            (
                "value3",
                Variant::from([
                    ("aaa", Variant::Integer(333)),
                    ("bbb", Variant::Integer(444)),
                    ("ccc", Variant::Integer(555)),
                    ("ddd", Variant::Integer(999)),
                ]),
            ),
            ("value4", Variant::Array(vec!["foo".into(), "foobar".into(), "bar".into()])),
        ])
        .as_object()
        .cloned()
        .unwrap();

        assert!(obj1.get("value3").unwrap().as_object().unwrap().contains_key("aaa"));
        let _ = obj1.remove_nav_property("value3.aaa").unwrap();
        assert!(!obj1.get("value3").unwrap().as_object().unwrap().contains_key("aaa"));

        assert!(obj1.get("value4").unwrap().as_array().unwrap().contains(&Variant::String("foobar".into())));
        assert_eq!(obj1.get("value4").unwrap().as_array().unwrap().len(), 3);
        let _ = obj1.remove_nav_property("value4[1]").unwrap();
        assert!(!obj1.get("value4").unwrap().as_array().unwrap().contains(&Variant::String("foobar".into())));
        assert_eq!(obj1.get("value4").unwrap().as_array().unwrap().len(), 2);
    }
}
