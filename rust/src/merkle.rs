use std::{fs, io, path::Path, path::PathBuf, sync::Arc};

pub struct Node {
    path: PathBuf,
    root_path: Arc<PathBuf>,
    #[allow(clippy::vec_box)]
    childs: Vec<Box<Node>>,
}

impl Node {
    pub fn new(path: PathBuf, root_path: Arc<PathBuf>) -> std::io::Result<Node> {
        let mut childs: Vec<Box<Node>> = Vec::new();

        for entry_res in std::fs::read_dir(&path)? {
            let entry = entry_res?; // 处理单个目录项的 io::Result
            let child_path = entry.path(); // 得到 PathBuf
            let child = Node::new(child_path, Arc::clone(&root_path))?;
            childs.push(Box::new(child));
        }

        Ok(Node {
            path,
            root_path,
            childs,
        })
    }

    pub fn hash(&self) -> io::Result<String> {
        // leaf node
        if self.childs.is_empty() {
            if self.path.is_file() {
                return hash_file_md5(&self.path);
                // 如果你想完全对齐 Python 的 hash_file(self.path, self.root_path)
                // 就把 hash_file_md5 换成你自己的 hash_file 实现即可
            } else {
                // empty folder: md5(relative path as posix)
                let rel = self
                    .path
                    .strip_prefix(self.root_path.as_path())
                    .unwrap_or(self.path.as_path());

                let rel_posix = path_to_posix(rel);
                let digest = md5::compute(rel_posix.as_bytes());
                return Ok(format!("{:x}", digest));
            }
        }

        // non-leaf: md5 over sorted children's hashes
        let mut idx: Vec<usize> = (0..self.childs.len()).collect();
        idx.sort_by(|&a, &b| {
            let pa = rel_to_parent(&self.childs[a].path, &self.path);
            let pb = rel_to_parent(&self.childs[b].path, &self.path);
            pa.cmp(&pb)
        });

        let mut ctx = md5::Context::new();
        for i in idx {
            let ch = &self.childs[i];
            let child_hash = ch.hash()?; // 递归
            ctx.consume(child_hash.as_bytes()); // python: h.update(child.hash.encode("utf-8"))
        }
        Ok(format!("{:x}", ctx.finalize()))
    }
}

// 把 child.path.relative_to(self.path) 变成一个可排序的字符串 key（用 posix 风格）
fn rel_to_parent(child: &Path, parent: &Path) -> String {
    let rel = child.strip_prefix(parent).unwrap_or(child);
    path_to_posix(rel)
}

// 把 Path 转成类似 Python as_posix() 的字符串：用 '/' 分隔
fn path_to_posix(p: &Path) -> String {
    // 用 to_string_lossy 处理非 utf-8 路径；并把 Windows 的 '\' 统一成 '/'
    p.to_string_lossy().replace('\\', "/")
}

fn hash_file_md5(path: &Path) -> io::Result<String> {
    use std::io::Read;

    let mut f = fs::File::open(path)?;
    let mut ctx = md5::Context::new();
    let mut buf = [0u8; 1024 * 64];

    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        ctx.consume(&buf[..n]);
    }

    Ok(format!("{:x}", ctx.finalize()))
}

pub struct FileMerkleTree {
    root_node: Node,
    root_path: Arc<PathBuf>,
}

impl FileMerkleTree {
    pub fn new(root_path: PathBuf) -> std::io::Result<FileMerkleTree> {
        let root_path = Arc::new(root_path);
        let root_node = Node::new((*root_path).clone(), Arc::clone(&root_path))?;
        Ok(FileMerkleTree {
            root_path,
            root_node,
        })
    }

    pub fn get_hash(&self) -> io::Result<String> {
        self.root_node.hash()
    }
}
