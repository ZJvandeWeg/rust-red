use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{alpha1, alphanumeric1, space0},
    combinator::recognize,
    multi::many0,
    sequence::{delimited, pair},
    IResult, Parser,
};

pub fn identifier(input: &str) -> IResult<&str, &str, nom::error::VerboseError<&str>> {
    recognize(pair(alt((alpha1, tag("_"))), many0(alt((alphanumeric1, tag("_")))))).parse(input)
}

pub fn identifier_token(input: &str) -> nom::IResult<&str, &str, nom::error::VerboseError<&str>> {
    delimited(space0, identifier, space0).parse(input)
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
        assert!(identifier_token("invalid-").is_err());
        assert!(identifier_token("invalid -").is_err());
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
