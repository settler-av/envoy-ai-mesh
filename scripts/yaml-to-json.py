#!/usr/bin/env python3
"""
Convert YAML config to JSON for njs consumption
This script can be used to pre-convert YAML config to JSON format
which is easier for njs to parse.
"""

import sys
import yaml
import json

def convert_yaml_to_json(yaml_file, json_file=None):
    """Convert YAML file to JSON"""
    try:
        with open(yaml_file, 'r') as f:
            data = yaml.safe_load(f)
        
        json_output = json.dumps(data, indent=2)
        
        if json_file:
            with open(json_file, 'w') as f:
                f.write(json_output)
            print(f"✓ Converted {yaml_file} to {json_file}")
        else:
            print(json_output)
        
        return True
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return False

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: yaml-to-json.py <yaml_file> [json_file]")
        sys.exit(1)
    
    yaml_file = sys.argv[1]
    json_file = sys.argv[2] if len(sys.argv) > 2 else None
    
    success = convert_yaml_to_json(yaml_file, json_file)
    sys.exit(0 if success else 1)

