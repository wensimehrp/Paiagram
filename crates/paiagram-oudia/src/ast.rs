use pest_consume::Parser;
use smallvec::SmallVec;
use std::borrow::Cow;

// Glue for bridging Pest and Rust.
pub mod oudia {
    use super::Structure;
    use pest_consume::{Error, Parser, match_nodes};

    #[derive(Parser)]
    #[grammar = "ast.pest"]
    pub struct OuDiaParser;

    type Result<T> = std::result::Result<T, Error<Rule>>;
    type Node<'i> = pest_consume::Node<'i, Rule, ()>;

    impl OuDiaParser {
        fn match_inner<'a>(nodes: impl Iterator<Item = Node<'a>>) -> Result<Vec<Structure<'a>>> {
            let mut v = Vec::with_capacity(nodes.size_hint().0);
            for it in nodes {
                match it.as_rule() {
                    Rule::kvpair => v.push(Self::kvpair(it)?),
                    Rule::r#struct => v.push(Self::r#struct(it)?),
                    rule => return Err(it.error(format!("unexpected rule: {:?}", rule))),
                }
            }
            Ok(v)
        }
    }

    #[pest_consume::parser]
    impl OuDiaParser {
        fn key_ident(input: Node<'_>) -> Result<&str> {
            Ok(input.as_str())
        }
        fn item(input: Node<'_>) -> Result<&str> {
            Ok(input.as_str())
        }
        fn value(input: Node<'_>) -> Result<impl Iterator<Item = &str>> {
            Ok(match_nodes!(input.into_children();
                [item(it)..] => it
            ))
        }
        fn kvpair(input: Node<'_>) -> Result<Structure<'_>> {
            Ok(match_nodes!(input.into_children();
                [key_ident(k), value(v)] => Structure::Pair(k.into(), v.map(|it| it.into()).collect())
            ))
        }
        fn struct_ident(input: Node<'_>) -> Result<&str> {
            Ok(input.as_str())
        }
        fn r#struct(input: Node<'_>) -> Result<Structure<'_>> {
            Ok(match_nodes!(input.into_children();
                [struct_ident(k), rest..] => Structure::Struct(k.into(), {
                    Self::match_inner(rest)?
                })
            ))
        }
        pub fn root(input: Node<'_>) -> Result<Vec<Structure<'_>>> {
            Self::match_inner(input.into_children())
        }
    }
}

/// The structure of the .oud/oud2 format.
#[derive(Debug, Clone, PartialEq)]
pub enum Structure<'a> {
    /// A struct. A struct have an identifier and children fields,
    /// which could be either a [`Self::Struct`], or a [`Self::Pair`]
    Struct(Cow<'a, str>, Vec<Structure<'a>>),
    /// A pair. A pair comes with an identifier and children values.
    Pair(Cow<'a, str>, SmallVec<[Cow<'a, str>; 1]>),
}

impl<'a> Structure<'a> {
    /// Produce the name of the current structure.
    pub fn name(&self) -> &str {
        match self {
            Self::Struct(n, _) | Self::Pair(n, _) => n.as_ref(),
        }
    }
    pub fn at<'s, I, S>(
        &'s self,
        hierarchy: impl IntoIterator<IntoIter = I>,
    ) -> std::vec::IntoIter<&'s Self>
    where
        I: Iterator<Item = S> + Clone,
        S: AsRef<str>,
    {
        let mut out = Vec::new();
        Self::at_impl(self, hierarchy, &mut out);
        out.into_iter()
    }

    fn at_impl<'s, I, S>(
        node: &'s Self,
        hierarchy: impl IntoIterator<IntoIter = I>,
        out: &mut Vec<&'s Self>,
    ) where
        I: Iterator<Item = S> + Clone,
        S: AsRef<str>,
    {
        let mut hierarchy = hierarchy.into_iter();
        let Some(level) = hierarchy.next() else {
            out.push(node);
            return;
        };
        if let Self::Struct(_, vals) = node {
            for child in vals.iter().filter(|it| it.name() == level.as_ref()) {
                Self::at_impl(child, hierarchy.clone(), out);
            }
        }
    }
}

/// Serialize a [`Structure`] or a list of [`Structure`].
pub trait SerializeToOud {
    /// Serialize the [`Structure`] given the buffer.
    fn serialize_oud_to(&self, buf: &mut impl std::io::Write) -> std::io::Result<()>;
    /// Serialize the [`Structure`] to a [`String`]
    fn to_oud_string(&self) -> std::io::Result<String> {
        let mut buf: Vec<u8> = Vec::new();
        self.serialize_oud_to(&mut buf)?;
        String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
    /// Serialize the [`Structure`] and encode it as Shift-JIS. Note that this
    /// method returns a [`Vec<u8>`], since standard [`String`] only holds UTF-8
    /// encoded info.
    fn to_shift_jis_string(&self) -> std::io::Result<Vec<u8>> {
        use encoding_rs::SHIFT_JIS;
        let s = self.to_oud_string()?;
        let (data, ..) = SHIFT_JIS.encode(&s);
        Ok(data.to_vec())
    }
}

impl<'a> SerializeToOud for Structure<'a> {
    fn serialize_oud_to(&self, buf: &mut impl std::io::Write) -> std::io::Result<()> {
        match self {
            Self::Struct(key, val) => {
                buf.write_all(key.as_bytes())?;
                buf.write_all(b".\n")?;
                val.serialize_oud_to(buf)?;
                buf.write_all(b".\n")?;
            }
            Self::Pair(key, val) => {
                buf.write_all(key.as_bytes())?;
                buf.write_all(b"=")?;
                if let Some((first, rest)) = val.split_first() {
                    buf.write_all(first.as_bytes())?;
                    for r in rest {
                        buf.write_all(b",")?;
                        buf.write_all(r.as_bytes())?;
                    }
                }
                buf.write_all(b"\n")?;
            }
        }
        Ok(())
    }
}

impl<'a> SerializeToOud for [Structure<'a>] {
    fn serialize_oud_to(&self, buf: &mut impl std::io::Write) -> std::io::Result<()> {
        for val in self.iter() {
            val.serialize_oud_to(buf)?;
        }
        Ok(())
    }
}

impl<T: SerializeToOud + ?Sized> SerializeToOud for &T {
    fn serialize_oud_to(&self, buf: &mut impl std::io::Write) -> std::io::Result<()> {
        T::serialize_oud_to(*self, buf)
    }
}

// 'a = the string data's life
// 'r = the temporary borrow's life
pub trait GetItemWithKey<'a, 'r>: Sized {
    fn every<'i>(self, index: &'i str) -> impl Iterator<Item = &'r Structure<'a>> + 'i + Clone
    where
        Self: 'i,
        'a: 'r; // The data 'a must live at least as long as the reference 'r
    fn struct_inner(self) -> impl Iterator<Item = &'r Structure<'a>> + Clone
    where
        'a: 'r;
    fn once(self, index: &str) -> Option<&'r Structure<'a>>
    where
        'a: 'r,
    {
        self.every(index).next()
    }
    fn every_struct<'i>(
        self,
        index: &'i str,
    ) -> impl Iterator<Item = (Cow<'a, str>, &'r [Structure<'a>])> + 'i + Clone
    where
        Self: 'i,
        'a: 'r,
        'r: 'i,
    {
        self.every(index).filter_map(|it| {
            if let Structure::Struct(key, fields) = it {
                Some((key.clone(), fields.as_slice()))
            } else {
                None
            }
        })
    }
    fn every_pair<'i>(
        self,
        index: &'i str,
    ) -> impl Iterator<Item = impl Iterator<Item = &'i str>> + 'i + Clone
    where
        Self: 'i,
        'a: 'r,
        'a: 'i,
        'r: 'i,
    {
        self.every(index).filter_map(|it| {
            if let Structure::Pair(_, values) = it {
                Some(values.iter().map(|it| it.as_ref()))
            } else {
                None
            }
        })
    }
}

impl<'a, 'r, T> GetItemWithKey<'a, 'r> for T
where
    T: Iterator<Item = &'r Structure<'a>> + Clone,
    'a: 'r,
{
    fn every<'i>(self, index: &'i str) -> impl Iterator<Item = &'r Structure<'a>> + 'i + Clone
    where
        Self: 'i,
    {
        self.filter(move |it| it.name() == index)
    }
    fn struct_inner(self) -> impl Iterator<Item = &'r Structure<'a>> + Clone
    where
        'a: 'r,
    {
        self.flat_map(|it| {
            if let Structure::Struct(_, fields) = it {
                fields.iter()
            } else {
                [].iter()
            }
        })
    }
}

/// Parses the input to an OuDia AST.
pub fn parse_to_ast(input: &str) -> Result<Vec<Structure<'_>>, pest::error::Error<oudia::Rule>> {
    let root = oudia::OuDiaParser::parse(oudia::Rule::root, input)?.single()?;
    oudia::OuDiaParser::root(root)
}

#[cfg(test)]
mod test {
    use super::*;
    type E = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_file_1() -> E {
        // Please make sure that the test files are LF ending, UTF-8 encoded
        let s = include_str!("../test/sample.oud2");
        let r = parse_to_ast(s)?;
        assert_eq!(s, r.to_oud_string()?);
        Ok(())
    }

    #[test]
    fn test_parse_file_2() -> E {
        let s = include_str!("../test/sample2.oud2");
        let r = parse_to_ast(s)?;
        assert_eq!(s, r.to_oud_string()?);
        Ok(())
    }
}
