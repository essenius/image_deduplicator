use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::fs::{self,File, OpenOptions};
use sha2::{Sha256, Digest};
use std::error::Error;
use filetime::FileTime;
use walkdir::{DirEntry, WalkDir};
use std::io::ErrorKind;


static DUPLICATE_EXTENSION: &str = "duplicate";

#[derive(Debug, Clone)]
struct ImageData {
    path: String,
    create_time: FileTime,
    size: u64,
    is_duplicate: bool,
}

fn hash(file: &String) -> Result<String, io::Error> {
    let path = Path::new(file);

    let mut file = File::open(&path)?;
    let mut sha256 = Sha256::new();
    io::copy(&mut file, &mut sha256).expect("copy failed");
    Ok(format!("{:x}", sha256.finalize()))
}

fn add_to_logfile(original: &String, duplicate: &String) {
    let dup_file = Path::new(duplicate);
    let logfile_path = dup_file.parent().unwrap().join("duplicates.log");
    //println!("{}",logfile_path.display());
    let logfile = OpenOptions::new()
            .append(true)
            .create(true)
            .open(logfile_path)
            .unwrap();
    let log_line = format!("{} is duplicate of {}", &duplicate, &original);
    writeln!(&logfile, "{}", &log_line).unwrap();    
    println!("{}", &log_line);    
}

fn get_create_time(metadata: &fs::Metadata) -> FileTime {
    let create_time : FileTime ;
    if let Some(time) = FileTime::from_creation_time(metadata) {
        create_time = time;
    } else {
        create_time = FileTime::from_last_modification_time(metadata);
    }
    return create_time;
}

fn correct_zero_modification_date(path: &Path, metadata: &fs::Metadata, create_time: &filetime::FileTime) {
    if  FileTime::from_last_modification_time(metadata)== FileTime::zero() {
        println!("Setting modified time to {}", create_time);
        
        filetime::set_file_mtime(path, *create_time).unwrap();
    }
}

fn is_duplicate(path: &Path) -> bool {
    let mut is_duplicate = false;
    if let Some(extension) = path.extension() {
        is_duplicate = extension == DUPLICATE_EXTENSION;
    }
    return is_duplicate;
}

fn is_hidden(entry: &DirEntry) -> bool {
    
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with("."))
         .unwrap_or(false)
}

fn images_in(folder: &Path) -> Vec<ImageData> {
    let mut images : Vec<ImageData> = Vec::new();
    let mut duplicate_count = 0;
    
    let mut walker = WalkDir::new(folder).into_iter();
    loop {
        let entry = match walker.next() {
            None => break,
            Some(Err(err)) => { 
                let path = err.path().unwrap_or(Path::new("")).display();
                if let Some(inner) = err.io_error() {
                    if inner.kind() == ErrorKind::PermissionDenied {
                        println!("Skipping {}: permission denied.", path);
                        continue;
                    }
                }
                panic!("ERROR: {}", err);
            }
            Some(Ok(entry)) => entry,
        };
        if entry.file_type().is_dir()  {
            if is_hidden(&entry) && entry.depth() > 0 {
                println!("hidden: {}",  entry.path().display());
                walker.skip_current_dir();
            } 
            continue;
        }
        let path = entry.path();
        let metadata = fs::metadata(&path).unwrap();

        let create_time = get_create_time(&metadata);
        correct_zero_modification_date(&path, &metadata, &create_time);
        let is_duplicate = is_duplicate(&path);
        if is_duplicate {
            duplicate_count +=1;
        }
        let name = format!("{}",path.display());
        images.push(ImageData{path: name.clone(), size: metadata.len(), create_time: create_time, is_duplicate: is_duplicate});
        print!("{}", if is_duplicate {"#"} else {"."});
        io::stdout().flush().unwrap();
    }
    println!("Found {} files, {} existing duplicates.", &images.len(), duplicate_count);
    return images;
}

fn mark_duplicates(images: &mut Vec<ImageData>) {
    let mut duplicate_count = 0;
    let mut duplicate_size = 0;
    let mut previous_percentage = 101;
    for i in 0..images.len() {
        let base_entry = images[i].clone();
        if base_entry.is_duplicate {
            continue;
        }
        // Per 5 percent (* 20 = * 100 /5)
        let percentage = (i * 20 / images.len()) * 5;
        if previous_percentage != percentage {
            println!("{}", format!("{}%", percentage));
            previous_percentage = percentage;
        }
        let mut j = i+1; 
        let mut base_hash_calculated = false;
        let mut base_hash = String::from("");
        while j < images.len() && images[j].size == base_entry.size {
            if !images[j].is_duplicate { 
                if !base_hash_calculated {
                    base_hash = hash(&base_entry.path).unwrap();
                    base_hash_calculated = true;
                }
                let potential_dup_hash = hash(&images[j].path).unwrap();
                if potential_dup_hash.eq(&base_hash) {
                    images[j].is_duplicate = true;
                    duplicate_count += 1;
                    duplicate_size += images[j].size;                
                    let new_duplicate_name = format!("{}.{}", &images[j].path, DUPLICATE_EXTENSION);
                    fs::rename(&images[j].path, &new_duplicate_name).unwrap(); 
                    add_to_logfile(&base_entry.path, &new_duplicate_name);
                }
            }
            j+=1;
        }
    }
    println!("New duplicates found: {}, total size: {}", duplicate_count, duplicate_size);
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let path = PathBuf::from(&args[1]);
    if !&path.exists() {
        println!("Path '{}' does not exist", path.clone().into_os_string().into_string().unwrap());
    }

    let mut images = images_in(&path);
    images.sort_by(|a, b| a.size.cmp(&b.size).then(a.create_time.cmp(&b.create_time)));
    mark_duplicates(&mut images);
    Ok(())
}
