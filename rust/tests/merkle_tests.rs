use dataspringflow_rs::merkle::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    fn temp_dir() -> TempDir {
        TempDir::new().expect("create temp dir")
    }

    fn write_file<P: AsRef<Path>>(path: P, content: &[u8]) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(content).expect("write file content");
    }

    fn build_tree(root: &Path) -> FileMerkleTree {
        FileMerkleTree::new(root.to_path_buf()).expect("build merkle tree")
    }

    fn hash_tree(root: &Path) -> HashRes {
        let mut tree = build_tree(root);
        tree.get_hash().expect("hash tree")
    }

    #[cfg(unix)]
    fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) {
        std::os::unix::fs::symlink(src, dst).expect("create file symlink");
    }

    #[cfg(unix)]
    fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) {
        std::os::unix::fs::symlink(src, dst).expect("create dir symlink");
    }

    #[test]
    fn empty_tree_hashes_to_stable_value() {
        let td = temp_dir();
        let mut tree = build_tree(td.path());

        let hash1 = tree.get_hash().expect("hash empty tree");

        let mut tree2 = build_tree(td.path());
        let hash2 = tree2.get_hash().expect("hash empty tree again");

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn single_file_tree_hash_is_stable() {
        let td = temp_dir();
        write_file(td.path().join("hello.txt"), b"hello world");

        let h1 = hash_tree(td.path());
        let h2 = hash_tree(td.path());

        assert_eq!(h1, h2);
    }

    #[test]
    fn same_content_but_different_path_produces_different_hash() {
        let td1 = temp_dir();
        let td2 = temp_dir();

        write_file(td1.path().join("a.txt"), b"same content");
        write_file(td2.path().join("nested/b.txt"), b"same content");

        let h1 = hash_tree(td1.path());
        let h2 = hash_tree(td2.path());

        assert_ne!(h1, h2);
    }

    #[test]
    fn file_order_does_not_change_root_hash() {
        let td1 = temp_dir();
        let td2 = temp_dir();

        write_file(td1.path().join("a.txt"), b"A");
        write_file(td1.path().join("b.txt"), b"B");
        write_file(td1.path().join("dir/c.txt"), b"C");

        write_file(td2.path().join("dir/c.txt"), b"C");
        write_file(td2.path().join("b.txt"), b"B");
        write_file(td2.path().join("a.txt"), b"A");

        let h1 = hash_tree(td1.path());
        let h2 = hash_tree(td2.path());

        assert_eq!(h1, h2);
    }

    #[test]
    fn nested_directory_tree_hashes_stably() {
        let td = temp_dir();

        write_file(td.path().join("root.txt"), b"root");
        write_file(td.path().join("sub/a.txt"), b"a");
        write_file(td.path().join("sub/deeper/b.txt"), b"b");

        let h1 = hash_tree(td.path());
        let h2 = hash_tree(td.path());

        assert_eq!(h1, h2);
    }

    #[test]
    fn directory_and_file_structure_affects_hash() {
        let td1 = temp_dir();
        let td2 = temp_dir();

        write_file(td1.path().join("x/y.txt"), b"content");

        write_file(td2.path().join("x.txt"), b"content");
        write_file(td2.path().join("x"), b"other");

        let h1 = hash_tree(td1.path());
        let h2 = hash_tree(td2.path());

        assert_ne!(h1, h2);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn symlink_to_file_is_hashed_successfully() {
        let td = temp_dir();
        let target = td.path().join("target.txt");
        let link = td.path().join("link.txt");

        write_file(&target, b"symlink target");
        symlink_file(&target, &link);

        let mut tree = build_tree(td.path());
        let hash = tree.get_hash().expect("hash tree with file symlink");
        assert_ne!(hash, [0u8; 32]);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn symlink_to_directory_is_hashed_successfully() {
        let td = temp_dir();
        let target_dir = td.path().join("real_dir");
        let link_dir = td.path().join("dir_link");

        fs::create_dir_all(&target_dir).expect("create target dir");
        write_file(target_dir.join("a.txt"), b"aaa");
        write_file(target_dir.join("b.txt"), b"bbb");

        symlink_dir(&target_dir, &link_dir);

        let mut tree = build_tree(td.path());
        let hash = tree.get_hash().expect("hash tree with dir symlink");
        assert_ne!(hash, [0u8; 32]);
    }

    #[cfg(unix)]
    #[test]
    fn circular_symlink_returns_error() {
        let td = temp_dir();
        let a = td.path().join("a");
        let b = td.path().join("b");

        fs::create_dir_all(&a).expect("create dir a");
        fs::create_dir_all(&b).expect("create dir b");

        symlink_dir(&b, a.join("to_b"));
        symlink_dir(&a, b.join("to_a"));

        let result = FileMerkleTree::new(td.path().to_path_buf());
        assert!(result.is_err(), "expected circular symlink to fail");
    }

    #[test]
    fn tree_entries_include_root_and_paths() {
        let td = temp_dir();
        write_file(td.path().join("alpha.txt"), b"alpha");
        fs::create_dir_all(td.path().join("nested")).expect("create nested dir");

        let tree = build_tree(td.path());
        assert!(
            tree.entries
                .iter()
                .any(|e| e.rel_path.as_os_str().is_empty()),
            "root entry should exist"
        );
        assert!(
            tree.entries
                .iter()
                .any(|e| e.rel_path == Path::new("alpha.txt")),
            "file entry should exist"
        );
        assert!(
            tree.entries
                .iter()
                .any(|e| e.rel_path == Path::new("nested")),
            "dir entry should exist"
        );
    }

    #[test]
    fn empty_directory_inside_tree_contributes_consistently() {
        let td = temp_dir();
        fs::create_dir_all(td.path().join("empty_dir")).expect("create empty dir");

        let h1 = hash_tree(td.path());
        let h2 = hash_tree(td.path());

        assert_eq!(h1, h2);
    }
}
