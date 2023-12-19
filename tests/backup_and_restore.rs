#[cfg(test)]

use bigarchiver::{backup,check};
use bigarchiver::finalizable::DataSink;

mod common;

use rand::RngCore;
use test_case::test_matrix;
use std::sync::atomic::AtomicI32;

static CNT: AtomicI32 = AtomicI32::new(0);

struct SinkToVector<'a> {
    incoming: Vec<u8>,
    etalon: &'a [u8]
}

impl DataSink for SinkToVector<'_> {
    fn add(&mut self, data: &[u8]) -> Result<(), String> {
        self.incoming.extend_from_slice(data);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), String> {
        assert_eq!(&self.incoming, self.etalon);
        Ok(())
    }
}

#[test_matrix(
    [10, 100, 1000], // input_size
    [10, 100, 1000], // auth_size
    [10, 100, 1000], // split_size
    [10, 100, 1000]  // buf_size
)]
fn backup_restore_all_ok(input_size: usize, auth_size: usize, split_size: usize, buf_size: usize) {
    let cnt = CNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let parent_dir = format!("/tmp/all_ok_{}", cnt);
    let _ = std::fs::remove_dir_all(&parent_dir);
    let _ = std::fs::create_dir(&parent_dir);
    let out_tpl = format!("{}/%%%%%%", &parent_dir);
    let out_cfg = format!("{}/000000.cfg", &parent_dir);

    let mut src: Vec<u8> = Vec::with_capacity(input_size);
    src.resize(input_size, 0);
    rand::thread_rng().fill_bytes(&mut src);

    backup(
        &src[..],
        "The Author",
        auth_size,
        split_size,
        &out_tpl,
        "secret",
        9,
        buf_size).unwrap();

    let src_unpacked = SinkToVector{ incoming: Vec::new(), etalon: &src };

    check(
        Some(src_unpacked),
        &out_cfg,
        "secret",
        buf_size, false).unwrap();

}
