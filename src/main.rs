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

#[derive(Clone)]
struct ImageData {
    path: String,
    create_time: FileTime,
    size: u64,
    is_duplicate: bool,
    hash: Option<String>,
}

impl ImageData {
    fn new(path: &Path) -> ImageData {
        let metadata = fs::metadata(&path).unwrap();
        let create_time = get_create_time(&metadata);
        correct_zero_modification_date(&path, &metadata, &create_time);
        let is_duplicate = is_duplicate(&path);
        let name = format!("{}", path.display());
        ImageData { path: name.clone(), size: metadata.len(), create_time: create_time, is_duplicate: is_duplicate, hash: None}
    }

    fn is_duplicate(&self) -> bool {
        let mut is_duplicate = false;
        if let Some(extension) = Path::new(&self.path).extension() {
            is_duplicate = extension == DUPLICATE_EXTENSION;
        }
        return is_duplicate;
    }

    fn mark_duplicate(&mut self) {
        self.is_duplicate = true;
        let new_duplicate_name = format!("{}.{}", &self.path, DUPLICATE_EXTENSION);
        println!("Renaming {} to {}", &self.path, &new_duplicate_name);
        fs::rename(&self.path, &new_duplicate_name).unwrap();
        self.path = new_duplicate_name;
    }

    /* fn full_name(&self) {
        format!("{}", Path::new(&self.path).display())
    } */

    fn hash(&mut self) -> Result<String, io::Error> {
        match &self.hash {
            None => {
                println!("Calculating hash for {}", &self.path);
                let path = Path::new(&self.path);    
                let mut file = File::open(&path)?;
                let mut sha256 = Sha256::new();
                io::copy(&mut file, &mut sha256).expect("copy failed");
                self.hash = Some(format!("{:x}",sha256.finalize()));
                Ok(self.hash.clone().unwrap())
            },
            Some(hash) => Ok(hash.clone()),
        }
    }
}

fn add_to_logfile(original: &String, duplicate: &String) {
    let dup_file = Path::new(duplicate);
    let logfile_path = dup_file.parent().unwrap().join("duplicates.log");
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
        let image = ImageData::new(&entry.path());
        if image.is_duplicate() {
            duplicate_count += 1;
            print!("#");
        } else {
            print!(".");
            images.push(image);
        }
        io::stdout().flush().unwrap();
    }
    println!(" Found {} files, excluding {} existing duplicates.", &images.len(), duplicate_count);
    return images;
}

fn mark_duplicates(images: &mut Vec<ImageData>) {
    let mut duplicate_count = 0;
    let mut duplicate_size = 0;
    let mut previous_percentage = 101;
    for base_entry in 0..images.len() {
        if images[base_entry].is_duplicate {
            continue;
        }
        // Per 5 percent (* 20 = * 100 /5)
        let percentage = (base_entry * 20 / images.len()) * 5;
        if previous_percentage != percentage {
            println!("{}", format!("{}%", percentage));
            previous_percentage = percentage;
        }
        let mut candidate_dup = base_entry + 1; 
        while candidate_dup < images.len() && images[candidate_dup].size == images[base_entry].size {
            if !images[candidate_dup].is_duplicate { 
                if images[candidate_dup].hash().unwrap().eq(&images[base_entry].hash().unwrap()) {
                    images[candidate_dup].mark_duplicate();
                    duplicate_count += 1;
                    duplicate_size += images[candidate_dup].size;                
                    add_to_logfile(&images[base_entry].path, &images[candidate_dup].path);
                }
            }
            candidate_dup += 1;
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
