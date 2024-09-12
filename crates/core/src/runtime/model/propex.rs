use std::fmt::Display;

use thiserror::Error;

use crate::text::nom_parsers;

use nom::{
    branch::alt,
    bytes::complete::escaped,
    character::complete::{alphanumeric1, char, digit1, multispace0, one_of},
    combinator::{map, map_res},
    error::{context, ParseError, VerboseError},
    multi::many0,
    sequence::{delimited, preceded, terminated},
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

#[derive(Debug, Clone, PartialEq)]
pub enum PropexSegment<'a> {
    Index(usize),
    Property(&'a str), // Use a reference to a string slice
    Nested(Vec<PropexSegment<'a>>),
}

impl<'a> Display for PropexSegment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropexSegment::Index(i) => write!(f, "[{}]", i),
            PropexSegment::Property(s) => write!(f, "[\"{}\"]", s),
            PropexSegment::Nested(n) => {
                write!(f, "[")?;
                for s in n.iter() {
                    write!(f, "{}", s)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl PropexSegment<'_> {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropexSegment::Property(prop) => Some(*prop),
            _ => None,
        }
    }

    pub fn as_index(&self) -> Option<usize> {
        match self {
            PropexSegment::Index(index) => Some(*index),
            _ => None,
        }
    }
}

pub fn token<'a, O, E: ParseError<&'a str>, G>(input: G) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    G: Parser<&'a str, O, E>,
{
    delimited(multispace0, input, multispace0)
}

fn parse_usize(input: &str) -> IResult<&str, usize, VerboseError<&str>> {
    context("usize", map_res(digit1, |s: &str| s.parse::<usize>())).parse(input)
}

fn string_content<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    escaped(alphanumeric1, '\\', one_of("\"n\\"))(i)
}

fn parse_string_literal(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    let single_quoted =
        delimited(preceded(multispace0, char('\'')), string_content, terminated(char('\''), multispace0));
    let double_quoted = delimited(preceded(multispace0, char('"')), string_content, terminated(char('"'), multispace0));
    context("quoted_string", alt((single_quoted, double_quoted))).parse(input)
}

fn first_direct_property(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    token(nom_parsers::identifier).map(PropexSegment::Property).parse(i)
}

fn first_property(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    context("first_property", alt((first_direct_property, quoted_property, index, nested))).parse(i)
}

fn quoted_property(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    delimited(token(char('[')), parse_string_literal, token(char(']'))).map(PropexSegment::Property).parse(i)
}

fn direct_property(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    context("direct_property", preceded(token(char('.')), token(nom_parsers::identifier)))
        .map(PropexSegment::Property)
        .parse(i)
}

fn subproperty(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    context("subproperty", alt((direct_property, quoted_property, index, nested))).parse(i)
}

fn index(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    context("index", delimited(token(char('[')), token(parse_usize), token(char(']'))))
        .map(PropexSegment::Index)
        .parse(i)
}

fn nested(i: &str) -> IResult<&str, PropexSegment, VerboseError<&str>> {
    let nested_parser = delimited(token(char('[')), expression, token(char(']')));
    map(nested_parser, PropexSegment::Nested)(i)
}

fn expression(input: &str) -> IResult<&str, Vec<PropexSegment>, VerboseError<&str>> {
    let (input, first) = first_property.parse(input)?;
    let (input, rest) = context("propex_expr", many0(subproperty)).parse(input)?;
    let mut result = Vec::with_capacity(rest.len() + 1);
    result.push(first);
    result.extend(rest);
    Ok((input, result))
}

pub fn parse(expr: &str) -> Result<Vec<PropexSegment>, PropexError> {
    if expr.is_empty() {
        return Err(PropexError::BadArguments);
    }
    match expression(expr) {
        Ok((_, segs)) => Ok(segs),
        Err(ve) => Err(PropexError::BadSyntax(ve.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_primitives_should_be_ok() {
        let expr = "['test1']";
        let (_, parsed) = quoted_property(expr).unwrap();
        assert_eq!(PropexSegment::Property("test1"), parsed);

        let expr = r#"["test1"]"#;
        let (_, parsed) = quoted_property(expr).unwrap();
        assert_eq!(PropexSegment::Property("test1"), parsed);

        let expr = "_test_1";
        let (_, parsed) = first_direct_property(expr).unwrap();
        assert_eq!(PropexSegment::Property("_test_1"), parsed);

        let expr = ".foobar123";
        let (_, parsed) = direct_property(expr).unwrap();
        assert_eq!(PropexSegment::Property("foobar123"), parsed);

        let expr = " [ 'aaa']";
        let (_, parsed) = quoted_property(expr).unwrap();
        assert_eq!(PropexSegment::Property("aaa"), parsed);

        let expr = "[ 123 ]";
        let (_, parsed) = index(expr).unwrap();
        assert_eq!(PropexSegment::Index(123), parsed);
    }

    #[test]
    fn parse_propex_should_be_ok() {
        let expr1 = r#"test1. hello .world['aaa'][333]["bb"].name_of"#;
        let segs = parse(expr1).unwrap();

        assert_eq!(7, segs.len());
        assert_eq!(PropexSegment::Property("test1"), segs[0]);
        assert_eq!(PropexSegment::Property("hello"), segs[1]);
        assert_eq!(PropexSegment::Property("world"), segs[2]);
        assert_eq!(PropexSegment::Property("aaa"), segs[3]);
        assert_eq!(PropexSegment::Index(333), segs[4]);
        assert_eq!(PropexSegment::Property("bb"), segs[5]);
        assert_eq!(PropexSegment::Property("name_of"), segs[6]);
    }

    #[test]
    fn parse_propex_with_first_index_accessing_should_be_ok() {
        let expr1 = r#"['test1'].hello .world['aaa'].see[333]["bb"].name_of"#;
        let segs = parse(expr1).unwrap();

        assert_eq!(8, segs.len());
        assert_eq!(PropexSegment::Property("test1"), segs[0]);
        assert_eq!(PropexSegment::Property("hello"), segs[1]);
        assert_eq!(PropexSegment::Property("world"), segs[2]);
        assert_eq!(PropexSegment::Property("aaa"), segs[3]);
        assert_eq!(PropexSegment::Property("see"), segs[4]);
        assert_eq!(PropexSegment::Index(333), segs[5]);
        assert_eq!(PropexSegment::Property("bb"), segs[6]);
        assert_eq!(PropexSegment::Property("name_of"), segs[7]);
    }

    #[test]
    fn parse_propex_with_nested_propex() {
        let expr1 = r#"['test1'].msg .payload[msg["topic"][0]].str[123]"#;
        let segs = parse(expr1).unwrap();
        dbg!(&segs);

        assert_eq!(6, segs.len());
        assert_eq!(PropexSegment::Property("test1"), segs[0]);
        assert_eq!(PropexSegment::Property("msg"), segs[1]);
        assert_eq!(PropexSegment::Property("payload"), segs[2]);
        assert_eq!(
            PropexSegment::Nested(vec![
                PropexSegment::Property("msg"),
                PropexSegment::Property("topic"),
                PropexSegment::Index(0)
            ]),
            segs[3]
        );
        assert_eq!(PropexSegment::Property("str"), segs[4]);
        assert_eq!(PropexSegment::Index(123), segs[5]);
    }
}
