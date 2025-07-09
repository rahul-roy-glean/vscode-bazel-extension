use prost::Message;
use anyhow::{Result, Context};
use std::collections::HashMap;

// Include the generated protobuf code
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/blaze.query.rs"));
}

use proto::{QueryResult, Target, Attribute};

pub struct QueryParser;

impl QueryParser {
    pub fn new() -> Self {
        Self
    }
    
    pub fn parse_proto_output(&self, data: &[u8]) -> Result<ParsedQueryResult> {
        let query_result = QueryResult::decode(data)
            .context("Failed to decode protobuf query result")?;
        
        let mut targets = Vec::new();
        
        for target in query_result.target {
            if let Some(parsed) = self.parse_target(target)? {
                targets.push(parsed);
            }
        }
        
        Ok(ParsedQueryResult { targets })
    }
    
    fn parse_target(&self, target: Target) -> Result<Option<ParsedTarget>> {
        match target.r#type() {
            proto::target::Discriminator::Unknown => Ok(None),
            proto::target::Discriminator::Rule => {
                if let Some(rule) = target.rule {
                    let mut attributes = HashMap::new();
                    
                    // Parse attributes
                    for attr in rule.attribute {
                        if let Some(value) = self.parse_attribute_value(&attr) {
                            attributes.insert(attr.name.clone(), value);
                        }
                    }
                    
                    Ok(Some(ParsedTarget {
                        name: rule.name,
                        kind: rule.rule_class,
                        inputs: rule.rule_input,
                        outputs: rule.rule_output,
                        attributes,
                    }))
                } else {
                    Ok(None)
                }
            }
            proto::target::Discriminator::SourceFile => {
                if let Some(source) = target.source_file {
                    Ok(Some(ParsedTarget {
                        name: source.name.clone(),
                        kind: "source_file".to_string(),
                        inputs: vec![],
                        outputs: vec![source.name],
                        attributes: HashMap::new(),
                    }))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None)
        }
    }
    
    fn parse_attribute_value(&self, attr: &Attribute) -> Option<AttributeValue> {
        use proto::attribute::Value;
        
        match &attr.value {
            Some(Value::StringValue(s)) => Some(AttributeValue::String(s.clone())),
            Some(Value::IntValue(i)) => Some(AttributeValue::Int(*i)),
            Some(Value::BoolValue(b)) => Some(AttributeValue::Bool(*b)),
            Some(Value::StringListValue(list)) => {
                Some(AttributeValue::StringList(list.string_value.clone()))
            }
            None => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedQueryResult {
    pub targets: Vec<ParsedTarget>,
}

#[derive(Debug, Clone)]
pub struct ParsedTarget {
    pub name: String,
    pub kind: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub attributes: HashMap<String, AttributeValue>,
}

#[derive(Debug, Clone)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Bool(bool),
    StringList(Vec<String>),
} 