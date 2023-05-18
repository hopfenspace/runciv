#!/usr/bin/env python3

import os
import pathlib


def main():
    script_file = pathlib.Path(__file__)
    project_root = script_file.parent.parent
    spec = project_root / "openapi.json"
    generated = project_root / "src" / "api" / "generated"
    config = generated / "config.json"

    command = f"npx @openapitools/openapi-generator-cli generate -g typescript-fetch -i {spec} -o {generated} -c {config}"
    print(command)
    os.system(command)


if __name__ == '__main__':
    main()
