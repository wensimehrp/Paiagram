use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "rw_data/oudiasecond.pest"]
pub struct OUD2Parser;

#[derive(Debug)]
enum OUD2Struct<'a> {
    Struct(&'a str, Vec<OUD2Struct<'a>>),
    Pair(&'a str, OUD2Value<'a>),
}

#[derive(Debug)]
enum OUD2Value<'a> {
    Single(&'a str),
    List(Vec<&'a str>),
}

fn parse_oud2_to_ast(file: &str) -> Result<OUD2Struct<'_>, pest::error::Error<Rule>> {
    let oud2 = OUD2Parser::parse(Rule::file, file)?
        .next()
        .unwrap()
        .into_inner()
        .next()
        .unwrap();
    use pest::iterators::Pair;
    fn parse_struct(pair: Pair<Rule>) -> OUD2Struct {
        match pair.as_rule() {
            Rule::r#struct => {
                let mut inner = pair.into_inner();
                let name = inner.next().unwrap().as_str();
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                OUD2Struct::Struct(name, fields)
            }
            Rule::wrapper => {
                let inner = pair.into_inner();
                let name = "file";
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                OUD2Struct::Struct(name, fields)
            }
            Rule::kvpair => {
                let mut inner = pair.into_inner();
                let key = inner.next().unwrap().as_str();
                let val = inner.next().unwrap();
                let val = match val.as_rule() {
                    Rule::value => OUD2Value::Single(val.as_str()),
                    Rule::list => {
                        let list_vals = val.into_inner().map(|v| v.as_str()).collect();
                        OUD2Value::List(list_vals)
                    }
                    _ => unreachable!(),
                };
                OUD2Struct::Pair(key, val)
            }
            _ => unreachable!(),
        }
    }
    Ok(parse_struct(oud2))
}

pub fn load_oud2() {

}
