use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{alpha1, alphanumeric1, space0},
    combinator::recognize,
    error::{ParseError, VerboseError},
    multi::many0,
    sequence::{delimited, pair},
    IResult, Parser,
};

pub fn spaces<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    let chars = " \t\r\n";

    // nom combinators like `take_while` return a function. That function is the
    // parser,to which we can pass the input
    take_while(move |c| chars.contains(c))(i)
}

pub fn identifier<'a>(input: &'a str) -> IResult<&'a str, &'a str, VerboseError<&'a str>> {
    recognize(pair(alt((alpha1, tag("_"))), many0(alt((alphanumeric1, tag("_")))))).parse(input)
}

pub fn identifier_token<'a>(input: &'a str) -> nom::IResult<&'a str, &'a str, VerboseError<&'a str>> {
    recognize(delimited(space0, identifier, space0)).parse(input)
}

fn is_identifier_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn identifier_while<'a>(input: &'a str) -> IResult<&'a str, &'a str, VerboseError<&'a str>> {
    recognize(pair(
        take_while1(is_identifier_start), // 起始字符必须是字母或下划线
        take_while1(is_identifier_char),  // 后续字符可以是字母、数字或下划线
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_identifiers() {
        assert_eq!(identifier("identifier"), Ok(("", "identifier")));
        assert_eq!(identifier("_underscore"), Ok(("", "_underscore")));
        assert_eq!(identifier("id123"), Ok(("", "id123")));
        assert_eq!(identifier("longer_identifier_with_123"), Ok(("", "longer_identifier_with_123")));
    }

    #[test]
    fn test_invalid_identifiers() {
        assert!(identifier("123start").is_err());
        assert!(identifier_token("-leading").is_err());
        assert!(identifier_while("invalid-").is_err());
        assert!(identifier_while("invalid -").is_err());
        assert!(identifier("").is_err());
    }

    #[test]
    fn test_identifier_edge_cases() {
        assert_eq!(identifier("_"), Ok(("", "_")));
        assert_eq!(identifier("a"), Ok(("", "a")));
        assert_eq!(identifier("a123"), Ok(("", "a123")));
        assert_eq!(identifier("a_b_c_123"), Ok(("", "a_b_c_123")));
    }
}
