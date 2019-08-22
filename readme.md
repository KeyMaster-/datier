# datier
Datier is a utility to rename all JPG and CR2 files in a folder based on their EXIF timestamps.

All files in a folder are grouped by their date, and then ordered by their timestamp on that day.  
The file name format is `yyyy_mm_dd-nnnn`, where nnnn is order number of the image within that day, starting at 1.

## Basic usage
Run `datier <path to folder>` to rename all JPG and CR2 files in that folder.  
See `datier --help` for additional commands.

## Install
Install [Rust](https://www.rust-lang.org/tools/install) (tested using rust version 1.36), then run:
```
git clone https://github.com/KeyMaster-/datier.git
cd datier
cargo run
```