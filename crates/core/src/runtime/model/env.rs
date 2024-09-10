use std::sync::{Arc, RwLock, Weak};

use dashmap::DashMap;
use nom;

use super::{ElementId, RedPropertyType, Variant};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EnvEntry {
    pub name: String,

    pub value: String,

    #[serde(alias = "type")]
    pub type_: RedPropertyType,
}

#[derive(Debug)]
pub struct EnvStore {
    pub parent: RwLock<Option<Weak<EnvStore>>>,
    pub children: DashMap<ElementId, Arc<EnvStore>>,
    pub envs: DashMap<String, Variant>,
}

#[derive(Debug)]
pub struct EnvStoreBuilder {
    parent: Option<Weak<EnvStore>>,
    children: DashMap<ElementId, Arc<EnvStore>>,
    envs: DashMap<String, Variant>,
}

impl EnvStoreBuilder {
    pub fn new() -> Self {
        Self {
            parent: None,
            children: DashMap::new(),
            envs: DashMap::new(),
        }
    }

    pub fn envs(mut self, envs: &[EnvEntry]) -> Self {
        self.envs.extend(
            envs.iter()
                .map(|x| (x.name.clone(), x.value.clone().into())),
        );
        self
    }

    pub fn parent(mut self, parent: Arc<EnvStore>) -> Self {
        self.parent = Some(Arc::downgrade(&parent));
        self
    }

    pub fn add_child(self, eid: ElementId, child: Arc<EnvStore>) -> Self {
        self.children.insert(eid, child);
        self
    }

    pub fn build(self) -> Arc<EnvStore> {
        let this = Arc::new(EnvStore {
            parent: RwLock::new(self.parent),
            children: DashMap::new(),
            envs: self.envs,
        });

        for mut child in self.children.iter_mut() {
            let mut cp = child
                .value_mut()
                .parent
                .write()
                .expect("We need the set the parent");
            *cp = Some(Arc::downgrade(&this));
        }

        this
    }
}

pub fn replace_vars<'a, F, R>(input: &'a str, converter: F) -> String
where
    F: Fn(&'a str) -> R,
    R: AsRef<str>,
{
    // 定义一个辅助函数，用于提取变量名
    fn variable_name(input: &str) -> nom::IResult<&str, &str> {
        nom::sequence::delimited(
            nom::bytes::complete::tag("${"), // 以 "${" 开头
            nom::sequence::preceded(
                nom::character::complete::space0,
                nom::bytes::complete::take_while(|c: char| c.is_alphanumeric() || c == '_'),
            ), // 读取变量名
            nom::sequence::preceded(
                nom::character::complete::space0,
                nom::bytes::complete::tag("}"),
            ), // 以 "}" 结束
        )(input)
    }

    let mut output = input.to_string();
    let mut remaining_input = input;

    // 继续解析直到输入字符串处理完毕
    while let Ok((remaining, var)) = variable_name(remaining_input) {
        let replacement = converter(var);
        output = output.replace(&format!("${{{}}}", var.trim()), replacement.as_ref());
        remaining_input = remaining;
    }

    output
}

pub fn parse_complex_env(expr: &str) -> Option<&str> {
    match parse_complex_env_internal(expr) {
        Ok((_, x)) => Some(x),
        Err(_) => None,
    }
}

fn parse_complex_env_internal(input: &str) -> nom::IResult<&str, &str> {
    nom::sequence::delimited(
        nom::bytes::complete::tag("${"), // 匹配起始符号 "${"
        nom::sequence::delimited(
            nom::character::complete::multispace0, // 可选的空白字符
            nom::bytes::complete::take_while(|c: char| c.is_alphanumeric() || c == '_'), // 匹配变量名
            nom::character::complete::multispace0, // 可选的空白字符
        ),
        nom::bytes::complete::tag("}"), // 匹配结束符号 "}"
    )(input)
}
