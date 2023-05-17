#! /usr/bin/env python3
"""
This is a terrible and severely overengineered script that takes a typescript interface
description (which is heavily used in the DAP specification) and performs a rudimentary
conversion into a rust struct. By no means a correct or complete conversion, but does
a good chunk of the busywork.
"""
import re

from pathlib import Path
from typing import Dict

from .gen_enum_from_strings import gen_enum_def

class RE:
  FIELD = re.compile(r"(?P<var>\w+)(?P<opt>\?)?: (?P<type>\w+)(?P<arr>\[\])?,")
  SNEK = re.compile(r"(?<!^)(?=[A-Z])")
  INTERFACE = re.compile(r"interface (?P<name>\w+)(.+?)? {")
  ENUM = re.compile(r"\s*(?P<var>\w+)(?P<opt>\?)?: (?=.*[|])(?P<variants>['\w |]+);")
  TYPE_TAG = re.compile(r"(event|command): .*")


RM_LINES_WITH = r"  /\*\*", r"  \*/", r"body: {", "};", RE.TYPE_TAG


RPL = {
  r"string": "String",
  r"boolean": "bool",
  r"number": "i64",
  r"\s*\* ": "  /// ",
  r";": ",",
  RE.INTERFACE.pattern: "pub struct \g<name> {",
}


def dict_replace(in_str: str, rpl_map: Dict[str, str]) -> str:
  value = in_str
  for pattern, rpl in rpl_map.items():
    value = re.sub(pattern, rpl, value)
  return value


def should_remove(line: str) -> bool:
  if line.strip() == "":
    return True
  for rm_word in RM_LINES_WITH:
    if re.search(rm_word, line):
      return True
  return False


def this_is_snek(name: str) -> str:
  return RE.SNEK.sub("_", name).lower()


def edit_field_line(line: str) -> str:
  if match := RE.FIELD.search(line):
    varname = this_is_snek(match.group("var"))
    typename = match.group("type")

    is_opt = match.group("opt") is not None
    is_arr = match.group("arr") is not None
    if is_arr:
      typename = f"Vec<{typename}>"
    if is_opt:
      typename = f"Option<{typename}>"
    return f"  pub {varname}: {typename},"
  return line


def rewrite_enums(lines: list[str]) -> Dict[str, str]:
  enums = {}
  struct_name: str | None = None
  for idx, line in enumerate(lines):
    if match := RE.INTERFACE.match(line):
      struct_name = match.group("name")
    elif match := RE.ENUM.match(line):
      varname = match.group("var")
      typename = f"{struct_name}{varname.capitalize()}"

      enum_def = gen_enum_def(typename, match.group("variants"))
      enums[typename] = enum_def

      is_opt = match.group("opt") is not None
      #is_arr = match.group("arr") is not None
      # if is_arr:
      #   typename = f"Vec<{typename}>"
      if is_opt:
        typename = f"Option<{typename}>"

      lines[idx] = f"  pub {varname}: {typename};"
  return enums


if __name__ == "__main__":
  from argparse import ArgumentParser
  parser = ArgumentParser()
  parser.add_argument("input_file", type=Path)
  args = parser.parse_args()

  input_file: Path = args.input_file
  lines = input_file.read_text().splitlines()

  enums = rewrite_enums(lines)
  for enum in enums.values():
    print(enum)

  print()

  for line in lines:
    if should_remove(line):
      continue
    line = dict_replace(line, RPL)
    line = edit_field_line(line)
    print(line)
