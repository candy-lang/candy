#! /usr/bin/env python3
"""
This is a terrible script that takes a typescript stringy enum declaration, such as

  "'normal' | 'emphasize' | 'deemphasize' | string"

and generates a Rust enum, along with a FromStr implementation.

By no means a correct or complete conversion, but does a good chunk of the busywork.
"""
import re
from io import StringIO

tmpl = """
#[derive(Debug)]
pub enum {name} {{
  {variants}
}}

impl FromStr for {name} {{
  type Err = DeserializationError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {{
    match s {{
      {match_arms}
    }}
  }}
}}

impl ToString for {name} {{
  fn to_string(&self) -> String {{
    match &self {{
      {enum2str_match_arms}
    }}
    .to_string()
  }}
}}

fromstr_deser!{{ {name} }}
tostr_ser!{{ {name} }}
"""

# impl<'de> Deserialize<'de> for {name} {{
#   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
#   where
#     D: Deserializer<'de>,
#   {{
#     let s = String::deserialize(deserializer)?;
#     FromStr::from_str(&s).map_err(de::Error::custom)
#   }}
# }}
# """


def get_strings(line: str) -> list[str]:
  return [re.search(r"'?(\w+)'?", item).group(1) for item in line.split("|")]


def cap(s: str) -> str:
  return s[0].upper() + s[1:]


def str_to_variant(s: str) -> str:
  parts = s.split(" ")
  return "".join(cap(w) for w in parts)


def get_variants(strings: list[str]) -> str:
  with StringIO() as strio:
    for s in strings:
      if s != "string":
        # print(f"  #[serde(rename = \"{s}\")]", file=strio)
        print(f"  {str_to_variant(s)},", file=strio)
      else:
        print(f"  String(String),", file=strio)
    return strio.getvalue().strip()


def get_match_arms(strings: list[str], enum_name: str) -> str:
  ind = "      "
  with StringIO() as strio:
    for item in strings:
      if item != "string":
        print(f'{ind}"{item}" => Ok({enum_name}::{str_to_variant(item)}),', file=strio)
      else:
        print(f"{ind}other => Ok({enum_name}::String(other.to_string()))", file=strio)
    if "string" not in strings:
      print(
        f"""{ind}other => Err(DeserializationError::StringToEnumParseError {{
        enum_name: "{enum_name}".to_string(),
        value: other.to_string(),
      }}),
        """,
        file=strio,
      )

    return strio.getvalue().strip()


def get_enum2str_match_arms(strings: list[str], enum_name: str) -> str:
  ind = "      "
  with StringIO() as strio:
    for item in strings:
      if item != "string":
        print(f'{ind}{enum_name}::{str_to_variant(item)} => "{item}",', file=strio)
      else:
        print(f"{ind}{enum_name}::String(other) => other", file=strio)

    return strio.getvalue().strip()


def gen_enum_def(enum_name: str, stringy_enum_line: str):
  strings = get_strings(stringy_enum_line)
  variants = get_variants(strings)
  match_arms = get_match_arms(strings, enum_name)

  return tmpl.format(
    name=enum_name,
    variants=variants,
    match_arms=match_arms,
    enum2str_match_arms=get_enum2str_match_arms(strings, enum_name),
  )


if __name__ == "__main__":
  from argparse import ArgumentParser

  parser = ArgumentParser()
  parser.add_argument("enum_name")
  parser.add_argument("stringy_enum_line")
  args = parser.parse_args()
  print(gen_enum_def(args.enum_name, args.stringy_enum_line))
