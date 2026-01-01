#!/usr/bin/env python3
"""
Consolidate all Module 3 task files into a single text document.
Captures filename, filepath, and contents with --- separators.
"""

import os
from pathlib import Path

def consolidate_files():
    # Get the directory where this script is located
    script_dir = Path(__file__).parent.resolve()

    # Define the files to consolidate (in order)
    files = [
        "_index.md",
        "_traceability.md",
        # Foundation layer
        "M03-F01.md", "M03-F02.md", "M03-F03.md", "M03-F04.md",
        "M03-F05.md", "M03-F06.md", "M03-F07.md", "M03-F08.md",
        "M03-F09.md", "M03-F10.md", "M03-F11.md", "M03-F12.md",
        "M03-F13.md", "M03-F14.md", "M03-F15.md", "M03-F16.md",
        "M03-F17.md",
        # Logic layer
        "M03-L01.md", "M03-L02.md", "M03-L03.md", "M03-L04.md",
        "M03-L05.md", "M03-L06.md", "M03-L07.md", "M03-L08.md",
        "M03-L09.md", "M03-L10.md", "M03-L11.md", "M03-L12.md",
        "M03-L13.md", "M03-L14.md", "M03-L15.md", "M03-L16.md",
        "M03-L17.md", "M03-L18.md", "M03-L19.md", "M03-L20.md",
        "M03-L21.md", "M03-L22.md", "M03-L23.md", "M03-L24.md",
        "M03-L25.md", "M03-L26.md", "M03-L27.md", "M03-L28.md",
        "M03-L29.md", "M03-L30.md", "M03-L31.md", "M03-L32.md",
        "M03-L33.md", "M03-L34.md", "M03-L35.md", "M03-L36.md",
        "M03-L37.md",
        # Surface layer
        "M03-S01.md", "M03-S02.md", "M03-S03.md", "M03-S04.md",
        "M03-S05.md", "M03-S06.md", "M03-S07.md", "M03-S08.md",
        "M03-S09.md", "M03-S10.md", "M03-S11.md", "M03-S12.md",
        "M03-S13.md", "M03-S14.md", "M03-S15.md", "M03-S16.md",
        "M03-S17.md", "M03-S18.md", "M03-S19.md", "M03-S20.md",
        "M03-S21.md", "M03-S22.md",
    ]

    output_path = script_dir / "module03_consolidated_tasks.txt"

    with open(output_path, 'w', encoding='utf-8') as outfile:
        outfile.write("=" * 80 + "\n")
        outfile.write("MODULE 3: 12-MODEL EMBEDDING PIPELINE - CONSOLIDATED TASK FILES\n")
        outfile.write("=" * 80 + "\n")
        outfile.write(f"Generated from: {script_dir}\n")
        outfile.write(f"Total files: {len(files)}\n")
        outfile.write("=" * 80 + "\n\n")

        files_processed = 0
        files_missing = []

        for filename in files:
            filepath = script_dir / filename

            if filepath.exists():
                # Read file contents
                with open(filepath, 'r', encoding='utf-8') as infile:
                    contents = infile.read()

                # Write file entry
                outfile.write(f"FILENAME: {filename}\n")
                outfile.write(f"FILEPATH: {filepath}\n")
                outfile.write("-" * 80 + "\n")
                outfile.write(contents)
                if not contents.endswith('\n'):
                    outfile.write('\n')
                outfile.write("\n---\n\n")

                files_processed += 1
                print(f"✓ Processed: {filename}")
            else:
                files_missing.append(filename)
                print(f"✗ Missing: {filename}")

        # Write summary at end
        outfile.write("=" * 80 + "\n")
        outfile.write("CONSOLIDATION SUMMARY\n")
        outfile.write("=" * 80 + "\n")
        outfile.write(f"Files processed: {files_processed}\n")
        outfile.write(f"Files missing: {len(files_missing)}\n")
        if files_missing:
            outfile.write(f"Missing files: {', '.join(files_missing)}\n")
        outfile.write("=" * 80 + "\n")

    print(f"\n{'=' * 60}")
    print(f"Consolidation complete!")
    print(f"Output: {output_path}")
    print(f"Files processed: {files_processed}/{len(files)}")
    if files_missing:
        print(f"Missing: {files_missing}")
    print(f"{'=' * 60}")

    return output_path

if __name__ == "__main__":
    consolidate_files()
