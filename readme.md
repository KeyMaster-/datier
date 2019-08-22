# datier
Datier is a utility to rename all JPG and CR2 files in a folder based on their EXIF timestamps.

All files in a folder are grouped by their date, and then ordered by their timestamp on that day.  
The file name format is "yyyy_mm_dd-nnnn", where nnnn is order number of the image within that day, starting at 1.

## Basic usage
Run `datier <path to folder>` to rename all JPG and CR2 files in that folder.  
See `datier --help` for additional commands.
