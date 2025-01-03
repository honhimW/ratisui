use anyhow::Result;
use clap::{arg, crate_version, ArgMatches, Command};
use std::collections::BTreeMap;

pub fn cli() -> Result<Command> {
    let command = Command::new("ratisui")
        .version(crate_version!())
        .about("Redis TUI build with Ratatui.")
        .args([
            arg!(-t --target <TARGET> "Named redis target in ~/.config/ratisui/databases.ron"),
            arg!(-T --theme <THEME> "Theme configuration in ~/.config/ratisui/theme/<THEME>.ron"),
        ]);

    Ok(command)
}

pub struct AppArguments {
    pub target: Option<String>,
    pub theme: Option<String>,
}

impl AppArguments {
    pub fn from_matches(arg_matches: &ArgMatches) -> Self {
        let values = Value::from_matches(arg_matches);
        let mut args = Self { target: None, theme: None };
        for (id, value) in values {
            if id == "target" {
                if let Value::String(s) = value {
                    args.target = Some(s);
                }
            } else if id == "theme" {
                if let Value::String(s) = value {
                    args.theme = Some(s);
                }
            }
        }
        args
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Value {
    Bool(bool),
    String(String),
}

impl Value {
    pub fn from_matches(matches: &ArgMatches) -> Vec<(clap::Id, Self)> {
        let mut values = BTreeMap::new();
        for id in matches.ids() {
            if matches.try_get_many::<clap::Id>(id.as_str()).is_ok() {
                // ignore groups
                continue;
            }
            if let Some(value_source) = matches.value_source(id.as_str()) {
                if value_source != clap::parser::ValueSource::CommandLine {
                    // Any other source just gets tacked on at the end (like default values)
                    continue;
                }
                if Self::extract::<String>(matches, id, &mut values) {
                    continue;
                }
                if Self::extract::<bool>(matches, id, &mut values) {
                    continue;
                }
                unimplemented!("unknown type for {id}: {matches:?}");
            }
        }
        values.into_values().collect::<Vec<_>>()
    }

    fn extract<T: Clone + Into<Value> + Send + Sync + 'static>(
        matches: &ArgMatches,
        id: &clap::Id,
        output: &mut BTreeMap<usize, (clap::Id, Self)>,
    ) -> bool {
        match matches.try_get_many::<T>(id.as_str()) {
            Ok(Some(values)) => {
                if let Some(indices) = matches.indices_of(id.as_str()) {
                    for (value, index) in values.zip(
                        indices,
                    ) {
                        output.insert(index, (id.clone(), value.clone().into()));
                    }
                }

                true
            }
            Ok(None) => {
                unreachable!("`ids` only reports what is present")
            }
            Err(clap::parser::MatchesError::UnknownArgument { .. }) => {
                unreachable!("id came from matches")
            }
            Err(clap::parser::MatchesError::Downcast { .. }) => false,
            Err(_) => {
                unreachable!("id came from matches")
            }
        }
    }
}

impl From<String> for Value {
    fn from(other: String) -> Self {
        Self::String(other)
    }
}

impl From<bool> for Value {
    fn from(other: bool) -> Self {
        Self::Bool(other)
    }
}