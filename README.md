# image_deduplicator
My project to start learning the Rust programming language. It's a utility that marks duplicate files in a folder structure.

The utility makes an inventory of the folder tree, and identifies duplicates by checking hashes for files with the same size. The oldest one is considered the original. 
When it does find a duplicate, it appends the extension '.duplicate' and adds it to a file 'duplicates.log' (along with the path of th efile it is a duplicate of) in 
the folder it found the duplicate in.
