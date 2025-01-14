mod hasher;
use hasher::DataHasher;

pub mod finalizable;
use finalizable::DataSink;

mod enc_dec;
use enc_dec::{Encryptor, Decryptor};

mod comp_decomp_2;
use comp_decomp_2::{Compressor2, Decompressor2};

mod fixed_size_writer;
use fixed_size_writer::FixedSizeWriter;

mod joiner;
use joiner::{Joiner,read_metadata};

mod multi_files_reader;
use multi_files_reader::MultiFilesReader;

mod buffered_reader;
use buffered_reader::BufferedReader;

mod stats;
use stats::Stats;

mod multi_files_writer;
use multi_files_writer::MultiFilesWriter;

mod splitter;
use splitter::Splitter;

pub mod arg_opts;
pub mod file_set;

mod free_space;
use free_space::get_free_space;

use std::time::{SystemTime, UNIX_EPOCH};
use std::io::Read;

pub fn backup<R: Read>(
    mut read_from: R,
    auth: &str, auth_every_bytes: usize, split_size_bytes: usize, out_template: &str, 
    pass: &str, compress_level: u8, buf_size_bytes: usize) -> Result<(), String>
{
    let hash_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap() // SAFE: rely on fact that now() cannot return anything earlier than EPOCH
        .as_secs();

    let mut stats = Stats::new();
    stats.auth_string = String::from(auth);
    stats.auth_chunk_size = auth_every_bytes;
    stats.out_chunk_size = Some(split_size_bytes);
    stats.hash_seed = Some(hash_seed);

    let mut fmgr = MultiFilesWriter::new();
    let mut spl: Splitter<'_, MultiFilesWriter> = Splitter::from_pattern(&mut fmgr, split_size_bytes, out_template)?;
    {
        let enc = Encryptor::new(&mut spl, pass, auth);
        let mut fbuf = FixedSizeWriter::new(enc, auth_every_bytes);
        let mut comp = Compressor2::new(&mut fbuf, compress_level as u32);
        {
            let mut hash_copier = DataHasher::with_writer(Some(&mut comp), hash_seed);

            let mut stdinbuf = BufferedReader::new(
                &mut read_from, &mut hash_copier, buf_size_bytes / 8, buf_size_bytes);

            stdinbuf.read_and_write_all()?;

            stats.in_data_len = Some(hash_copier.counter());
            stats.in_data_hash = Some(hash_copier.result());
        }
        stats.compressed_len = Some(comp.compressed());
    }

    spl.write_metadata(&stats)
}

    
pub fn check<W: DataSink>(mut write_to: Option<W>, cfg_path: &str, pass: &str, buf_size_bytes: usize, check_free_space: &Option<&str>) -> Result<(), String> {
    let stats = read_metadata::<MultiFilesReader>(cfg_path)?;
    eprintln!("authentication string: {}", stats.auth_string);

    if let Some(mount_point) = check_free_space {
        let all_data = stats.in_data_len.unwrap(); // SAFE because if was checked in read_metadata()
        if get_free_space(mount_point)? < all_data {
            return Err(format!("filesystem of '{}' won't fit {} of data to restore", mount_point, all_data));
        }
    }

    let ref_write_to = write_to.as_mut();

    let mut hash_copier = DataHasher::with_writer(ref_write_to, stats.hash_seed.unwrap());
    {
        let mut decomp = Decompressor2::new(&mut hash_copier);
        let dec = Decryptor::new(&mut decomp, pass, &stats.auth_string);
        let mut fbuf = FixedSizeWriter::new(dec, stats.auth_chunk_size + 16);
        let fmgr = MultiFilesReader::new();

        let mut joiner = Joiner::from_metadata(
            fmgr, &mut fbuf, cfg_path, buf_size_bytes)?;

        joiner.read_and_write_all()?;
    }

    if hash_copier.result() != stats.in_data_hash.unwrap() { // SAFE: read_metadata checked that all is set
        Err("hash verification error".to_owned())
    } else {
        Ok(())
    }
}
