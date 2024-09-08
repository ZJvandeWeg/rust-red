use std::fmt::Display;

use smallvec::SmallVec;
use thiserror::Error;

use nom::{
    branch::alt,
    bytes::complete::escaped,
    character::complete::{alphanumeric1, char, digit1, multispace0, one_of},
    combinator::{cut, map, map_res},
    error::{context, ParseError, VerboseError},
    multi::many0,
    sequence::{delimited, pair, preceded, terminated},
    IResult, Parser,
};

#[derive(Error, Debug)]
pub enum PropexError {
    #[error("Invalid arguments")]
    BadArguments,

    #[error("Invalid Propex syntax")]
    BadSyntax(String),

    #[error("Invalid number digit")]
    InvalidDigit,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PropexSegment<'a> {
    IntegerIndex(usize),
    StringIndex(&'a str), // Use a reference to a string slice
}

impl<'a> Display for PropexSegment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropexSegment::IntegerIndex(i) => write!(f, "[{}]", i),
            PropexSegment::StringIndex(s) => write!(f, ".{}", s),
        }
    }
}

impl PropexSegment<'_> {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropexSegment::StringIndex(prop) => Some(*prop),
            _ => None,
        }
    }

    pub fn as_index(&self) -> Option<usize> {
        match self {
            PropexSegment::IntegerIndex(index) => Some(*index),
            _ => None,
        }
    }
}

fn parse_double_quoted_string(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    map(
        context(
            "double_quoted_string",
            preceded(char('\"'), cut(terminated(parse_str, char('\"')))),
        ),
        PropexSegment::StringIndex,
    )
    .parse(i)
}

fn parse_single_quoted_string(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    map(
        context(
            "double_quoted_string",
            preceded(char('\''), cut(terminated(parse_str, char('\'')))),
        ),
        PropexSegment::StringIndex,
    )
    .parse(i)
}

fn parse_quoted_string(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    alt((parse_double_quoted_string, parse_single_quoted_string)).parse(i)
}

fn parse_str<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    escaped(alphanumeric1, '\\', one_of("\"n\\"))(i)
}

fn parse_integer_index(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    map_res(digit1, |digit_str: &str| {
        digit_str.parse::<usize>().map(PropexSegment::IntegerIndex)
    })
    .parse(i)
}

fn parse_index(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    delimited(
        delimited(multispace0, char('['), multispace0),
        alt((parse_quoted_string, parse_integer_index)),
        delimited(multispace0, char(']'), multispace0),
    )
    .parse(i)
}

fn parse_property(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    map(
        context(
            "property",
            delimited(
                multispace0,
                crate::text::nom_parsers::identifier,
                multispace0,
            ),
        ),
        PropexSegment::StringIndex,
    )
    .parse(i)
}

fn parse_subproperty(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    map(
        context(
            "subproperty",
            preceded(
                delimited(multispace0, char('.'), multispace0),
                delimited(
                    multispace0,
                    crate::text::nom_parsers::identifier,
                    multispace0,
                ),
            ),
        ),
        PropexSegment::StringIndex,
    )
    .parse(i)
}

fn parse_first_fragment(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    alt((parse_property, parse_index)).parse(i)
}

fn parse_sub_fragment(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    alt((parse_subproperty, parse_index)).parse(i)
}

fn parse_nav(i: &str) -> IResult<&str, SmallVec<[PropexSegment; 4]>, VerboseError<&str>> {
    let (_, (first, rest)) = pair(parse_first_fragment, many0(parse_sub_fragment)).parse(i)?;
    let mut segs = SmallVec::with_capacity(rest.len() + 1);
    segs.push(first);
    segs.extend(rest.into_iter());
    Ok((i, segs))
}

pub fn parse<'a>(expr: &'a str) -> Result<SmallVec<[PropexSegment<'a>; 4]>, PropexError> {
    if expr.is_empty() {
        return Err(PropexError::BadArguments);
    }
    match parse_nav(expr) {
        Ok((_, segs)) => Ok(segs),
        Err(ve) => Err(PropexError::BadSyntax(ve.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_primitives_should_be_ok() {
        let expr = "'test1'";
        let (_, parsed) = parse_single_quoted_string(expr).unwrap();
        assert_eq!(PropexSegment::StringIndex("test1"), parsed);

        let expr = "\"test1\"";
        let (_, parsed) = parse_double_quoted_string(expr).unwrap();
        assert_eq!(PropexSegment::StringIndex("test1"), parsed);

        let expr = "_test_1";
        let (_, parsed) = parse_property(expr).unwrap();
        assert_eq!(PropexSegment::StringIndex("_test_1"), parsed);

        let expr = " [ 'aaa']";
        let (_, parsed) = parse_index(expr).unwrap();
        assert_eq!(PropexSegment::StringIndex("aaa"), parsed);

        let expr = "[ 123 ]";
        let (_, parsed) = parse_index(expr).unwrap();
        assert_eq!(PropexSegment::IntegerIndex(123), parsed);
    }

    #[test]
    fn parse_propex_should_be_ok() {
        let expr1 = "test1.hello .world['aaa'][333][\"bb\"].name_of";
        let segs = parse(expr1).unwrap();

        assert_eq!(7, segs.len());
        assert_eq!(PropexSegment::StringIndex("test1"), segs[0]);
        assert_eq!(PropexSegment::StringIndex("hello"), segs[1]);
        assert_eq!(PropexSegment::StringIndex("world"), segs[2]);
        assert_eq!(PropexSegment::StringIndex("aaa"), segs[3]);
        assert_eq!(PropexSegment::IntegerIndex(333), segs[4]);
        assert_eq!(PropexSegment::StringIndex("bb"), segs[5]);
        assert_eq!(PropexSegment::StringIndex("name_of"), segs[6]);
    }

    #[test]
    fn parse_propex_with_first_index_accessing_should_be_ok() {
        let expr1 = "['test1'].hello .world['aaa'].see[333][\"bb\"].name_of";
        let segs = parse(expr1).unwrap();

        assert_eq!(8, segs.len());
        assert_eq!(PropexSegment::StringIndex("test1"), segs[0]);
        assert_eq!(PropexSegment::StringIndex("hello"), segs[1]);
        assert_eq!(PropexSegment::StringIndex("world"), segs[2]);
        assert_eq!(PropexSegment::StringIndex("aaa"), segs[3]);
        assert_eq!(PropexSegment::StringIndex("see"), segs[4]);
        assert_eq!(PropexSegment::IntegerIndex(333), segs[5]);
        assert_eq!(PropexSegment::StringIndex("bb"), segs[6]);
        assert_eq!(PropexSegment::StringIndex("name_of"), segs[7]);
    }
}
