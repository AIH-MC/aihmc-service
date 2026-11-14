"""Convert the existing flat/tables/source-list config without printing secrets."""

import argparse
import json
import tomllib
from pathlib import Path

import yaml


def encode_toml(config):
    lines = []
    for key, value in config.items():
        is_table_list = isinstance(value, list) and value and isinstance(value[0], dict)
        if not isinstance(value, dict) and not is_table_list:
            lines.append(f"{key} = {json.dumps(value, ensure_ascii=False)}")
    for key, value in config.items():
        if isinstance(value, dict):
            lines.append(f"\n[{key}]")
            lines.extend(
                f"{name} = {json.dumps(item, ensure_ascii=False)}"
                for name, item in value.items()
            )
        elif isinstance(value, list) and value and isinstance(value[0], dict):
            for item in value:
                lines.append(f"\n[[{key}]]")
                lines.extend(
                    f"{name} = {json.dumps(field, ensure_ascii=False)}"
                    for name, field in item.items()
                )
    text = "\n".join(lines) + "\n"
    if tomllib.loads(text) != config:
        raise ValueError(
            "Configuration could not be represented without changing values"
        )
    return text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", type=Path, default=Path("config.yml"))
    parser.add_argument(
        "destination", nargs="?", type=Path, default=Path("config.toml")
    )
    arguments = parser.parse_args()
    data = yaml.safe_load(arguments.source.read_text(encoding="utf-8"))
    text = encode_toml(data)
    # Never overwrite a configuration edited by the operator.
    with arguments.destination.open("x", encoding="utf-8", newline="\n") as output:
        output.write(text)
    print(
        f"Converted {arguments.source} -> {arguments.destination}; values not displayed"
    )


if __name__ == "__main__":
    main()
