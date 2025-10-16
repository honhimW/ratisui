use std::fs;
use std::path::Path;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use serde_json::Value;

/// Utility
fn main() -> Result<()> {
    // git clone repository into fs: https://github.com/redis/docs
    let repo_path = Path::new("E:\\projects\\redis-docs");
    let content_commands_path = repo_path.join("content").join("commands");
    if !content_commands_path.is_dir() {
        return Err(anyhow!("Expect a repository dir"));
    }
    let content_commands_dir = fs::read_dir(content_commands_path)?;

    let mut array: Vec<Value> = vec![];

    for command_entry_result in content_commands_dir {
        let command_entry = command_entry_result?;
        let command_path = command_entry.path();
        if let Some(file_name) = command_path.file_name() && file_name.to_string_lossy().ends_with(".md") {
            println!("{:?}", file_name);
            let raw = fs::read_to_string(command_path)?;
            let raw = get_middle_content(raw);
            let v: Value = serde_yaml::from_str(raw.as_str())?;
            array.push(build_json(v));
        }
    }

    let json = serde_json::to_string(&array)?;
    fs::write(Path::new("E:\\temp\\redis-cmd-rs.json"), json)?;

    let json = serde_json::to_string_pretty(&array)?;
    fs::write(Path::new("E:\\temp\\pretty-redis-cmd-rs.json"), json)?;

    Ok(())
}

fn get_middle_content(input: String) -> String {
    let first_index = input.find("---").map_or(0, |i| i + 3);
    let second_index = input[first_index..].find("---").map_or(0, |i| first_index + i);
    input[first_index..second_index].to_string()
}

fn build_json(v: Value) -> Value {
    let mut object = serde_json::Map::<String, Value>::new();
    object.insert("syntax".to_string(), v.get("syntax_fmt").cloned().unwrap_or_default());
    object.insert("command".to_string(), v.get("title").cloned().unwrap_or_default());
    object.insert("summary".to_string(), v.get("summary").cloned().unwrap_or_default());
    object.insert("since".to_string(), v.get("since").cloned().unwrap_or_default());
    object.insert("complexity".to_string(), v.get("complexity").cloned().unwrap_or_default());
    object.insert("group".to_string(), v.get("group").cloned().unwrap_or_default());

    let acl = v.get("acl_categories").map(|_v| {
        if let Value::Array(acl_categories) = _v {
            return  acl_categories.iter().map(|category| {
                if let Value::String(s) = category {
                    return s.clone();
                }
                category.to_string()
            }).join(", ");
        }
        String::new()
    }).unwrap_or_default();
    object.insert("acl".to_string(), Value::String(acl));

    let mut args: Vec<Value> = vec![];
    if let Some(_v) = v.get("arguments") {
        if let Value::Array(arguments) = _v {
            for argument in arguments {
                let mut arg = serde_json::Map::<String, Value>::new();
                let token_node = argument.get("token");
                let multiple_node = argument.get("multiple");
                let type_node = argument.get("type");
                let arguments_node = argument.get("arguments");
                let name_node = argument.get("name");
                let _display_text_node = argument.get("display_text");

                let argument_type = type_node.map(|_t| _t.as_str().map(|_s| _s.to_string()).unwrap_or_default()).unwrap_or_default();
                if let Some(__v) = token_node {
                    if "pure-token".eq_ignore_ascii_case(argument_type.as_str()) {
                        arg.insert("type".to_string(), Value::String("flag".to_string()));
                        arg.insert("value".to_string(), __v.clone());
                    } else {
                        arg.insert("type".to_string(), Value::String("arg".to_string()));
                        arg.insert("key".to_string(), __v.clone());
                        arg.insert("arg".to_string(), name_node.cloned().unwrap_or_default());
                        arg.insert("detail".to_string(), name_node.cloned().unwrap_or_default());
                    }
                } else if multiple_node.is_some() {
                    arg.insert("type".to_string(), Value::String("many".to_string()));
                    if let Some(arg_node) = arguments_node {
                        if let Value::Array(nested_arguments) = arg_node {
                            let name = nested_arguments.iter().map(|nested_argument| {
                                if let Some(_name) = nested_argument.get("name") {
                                    if let Value::String(s) = _name {
                                        return s.clone();
                                    }
                                }
                                String::new()
                            }).filter(|s| !s.is_empty()).join(" ");
                            arg.insert("name".to_string(), Value::String(name));
                        }
                    } else {
                        arg.insert("name".to_string(), name_node.cloned().unwrap_or_default());
                    }
                } else if arguments_node.is_some() {
                    if let Some(arg_node) = arguments_node {
                        if let Value::Array(nested_arguments) = arg_node {
                            let mut enum_values: Vec<Value> = vec![];
                            for nexted_argument in nested_arguments {
                                let token_string = if let Some(_token) = nexted_argument.get("token") {
                                    _token.as_str().unwrap_or_default().to_string()
                                } else {
                                    String::new()
                                };
                                let token = Value::String(token_string);
                                enum_values.push(token);
                            }
                            arg.insert("type".to_string(), Value::String("enum".to_string()));
                            arg.insert("values".to_string(), Value::Array(enum_values));
                        }
                    } else {
                        arg.insert("name".to_string(), name_node.cloned().unwrap_or_default());
                    }
                } else {
                    arg.insert("type".to_string(), Value::String("single".to_string()));
                    arg.insert("name".to_string(), name_node.cloned().unwrap_or_default());
                }
                args.push(Value::Object(arg));
            }
        }
    }
    object.insert("arguments".to_string(), Value::Array(args));

    Value::Object(object)
}
