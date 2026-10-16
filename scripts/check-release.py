#!/usr/bin/env python3
"""Fail CI on version drift, tag mismatch, or incomplete release artifacts."""
import argparse
from pathlib import Path
from release_support import check_versions, verify_artifacts

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--tag')
parser.add_argument('--artifacts', type=Path)
parser.add_argument('--commit')
args = parser.parse_args()
if args.artifacts:
    if not args.commit or not args.tag:
        parser.error('--artifacts requires --commit and --tag')
    verify_artifacts(args.artifacts, args.commit, args.tag)
else:
    print(f'Application version: {check_versions(tag=args.tag)}')
