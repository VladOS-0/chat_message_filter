use std::{fs::read_to_string, path::Path};

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    regex: bool,
    include: Option<String>,
    exclude: Option<String>,
    match_case: bool,

    // compiled regexes
    #[serde(skip)]
    include_regex: Option<Regex>,
    #[serde(skip)]
    exclude_regex: Option<Regex>,
}

impl Config {
    pub fn from_args(
        regex: bool,
        include: Option<String>,
        exclude: Option<String>,
        match_case: bool,
    ) -> anyhow::Result<Self> {
        let mut config = Self {
            regex,
            include,
            exclude,
            match_case,
            include_regex: None,
            exclude_regex: None,
        };
        if !match_case {
            config.include = config
                .include
                .and_then(|pattern| Some(pattern.to_lowercase()));

            config.exclude = config
                .exclude
                .and_then(|pattern| Some(pattern.to_lowercase()));
        }
        if regex {
            config.compile_regexes()?;
        }
        Ok(config)
    }

    fn compile_regexes(&mut self) -> anyhow::Result<()> {
        if let Some(include) = &self.include {
            self.include_regex = Some(Regex::new(include).map_err(|err| {
                anyhow::format_err!("failed to compile include regex from {}: {}", include, err)
            })?);
        };

        if let Some(exclude) = &self.exclude {
            self.exclude_regex = Some(Regex::new(exclude).map_err(|err| {
                anyhow::format_err!("failed to compile exclude regex from {}: {}", exclude, err)
            })?);
        };
        Ok(())
    }

    pub fn load<T: AsRef<Path>>(path: T) -> anyhow::Result<Self> {
        let toml_string = read_to_string(path).map_err(anyhow::Error::from)?;
        let mut config: Self = toml::from_str(&toml_string).map_err(anyhow::Error::from)?;

        if config.regex {
            config.compile_regexes()?;
        }

        Ok(config)
    }

    pub fn matches<T: AsRef<str>>(&self, haystack: T) -> Result<bool, anyhow::Error> {
        let haystack = if self.match_case {
            haystack.as_ref().to_string()
        } else {
            haystack.as_ref().to_lowercase()
        };

        if self.exclude.is_none() && self.include.is_none() {
            Err(anyhow::Error::msg(
                "no exclude/include patterns were provided",
            ))?
        }

        if let Some(include_regex) = &self.include_regex {
            if !include_regex.is_match(&haystack) {
                return Ok(false);
            }
        } else if let Some(include) = &self.include {
            if !haystack.contains(include) {
                return Ok(false);
            }
        }

        if let Some(exclude_regex) = &self.exclude_regex {
            if exclude_regex.is_match(&haystack) {
                return Ok(false);
            }
        } else if let Some(exclude) = &self.exclude {
            if haystack.contains(exclude) {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
