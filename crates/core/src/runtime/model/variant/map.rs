use super::*;

pub type VariantMap = BTreeMap<String, Variant>;

pub trait VariantMapExt {
    fn contains_property(&self, prop: &str) -> bool;
    fn get_property(&self, prop: &str) -> Option<&Variant>;
    fn get_property_mut(&mut self, prop: &str) -> Option<&mut Variant>;
    fn get_nav_property(&self, self_name: &str, expr: &str) -> Option<&Variant>;
    fn get_nav_property_mut(&mut self, self_name: &str, expr: &str) -> Option<&mut Variant>;
    fn set_property(&mut self, prop: String, value: Variant);
    fn set_nav_property(&mut self, expr: &str, value: Variant, create_missing: bool) -> crate::Result<()>;

    fn get_seg_property(&self, segs: &[PropexSegment]) -> Option<&Variant>;
    fn get_seg_property_mut(&mut self, segs: &[PropexSegment]) -> Option<&mut Variant>;
    fn normalize_segments<'a>(&'a self, self_name: &str, segs: &mut [PropexSegment<'a>]) -> crate::Result<()>;
}

impl VariantMapExt for VariantMap {
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
        self.normalize_segments(self_name, &mut segs).ok()?;
        self.get_seg_property(&segs)
    }

    fn get_nav_property_mut(&mut self, self_name: &str, expr: &str) -> Option<&mut Variant> {
        let mut segs = propex::parse(expr).ok()?;
        if segs.iter().any(|x| matches!(x, PropexSegment::Nested(_))) {
            // Do things
            self.normalize_segments(self_name, &mut segs).ok()?;
            let mut normalized = String::new();
            for seg in segs {
                write!(&mut normalized, "{}", seg).unwrap();
            }
            let segs = propex::parse(&normalized).ok()?;
            let segs = segs.clone();
            self.get_seg_property_mut(&segs)
        } else {
            self.get_seg_property_mut(&segs)
        }
    }

    fn set_property(&mut self, prop: String, value: Variant) {
        let _ = self.insert(prop, value);
    }

    fn set_nav_property(&mut self, expr: &str, value: Variant, create_missing: bool) -> crate::Result<()> {
        if expr.is_empty() {
            return Err(crate::EdgelinkError::BadArguments("The argument expr cannot be empty".to_string()).into());
        }

        let segs = propex::parse(expr).map_err(|e| crate::EdgelinkError::BadArguments(e.to_string()))?;

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
            [PropexSegment::Property(first_prop_name)] => self.get(*first_prop_name),
            [PropexSegment::Property(first_prop_name), ref rest @ ..] => {
                self.get(*first_prop_name)?.get_item_by_propex_segments(rest)
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

    fn normalize_segments<'a>(&'a self, self_name: &str, segs: &mut [PropexSegment<'a>]) -> crate::Result<()> {
        for seg in segs.iter_mut() {
            if let PropexSegment::Nested(nested_segs) = seg {
                if nested_segs.first() != Some(&PropexSegment::Property(self_name)) {
                    return Err(
                        EdgelinkError::BadArguments(format!("The expression must contains `{}.`", self_name)).into()
                    );
                }
                *seg = match self.get_seg_property(&nested_segs[1..]).ok_or(EdgelinkError::OutOfRange)? {
                    Variant::String(str_index) => PropexSegment::Property(str_index.as_str()),
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
}
