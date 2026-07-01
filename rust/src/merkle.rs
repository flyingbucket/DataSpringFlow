use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

pub struct FileMerkleTree {
    pub root_path: Arc<PathBuf>,
    pub entries: Vec<FileEntry>,
}

pub struct FileEntry {
    pub rel_path: PathBuf,
    pub file_type: fs::FileType,
    pub hash: String,
}

impl FileMerkleTree {
    pub fn new(root_path: PathBuf) -> std::io::Result<Self> {
        let root_arc = Arc::new(root_path);
        let mut entries = Vec::new();

        for entry in WalkDir::new(&*root_arc).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            let rel_path = path
                .strip_prefix(&*root_arc)
                .map(|p| p.to_path_buf())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            entries.push(FileEntry {
                rel_path,
                file_type: entry.file_type(),
                hash: String::new(),
            });
        }

        Ok(FileMerkleTree {
            root_path: root_arc,
            entries,
        })
    }

    fn hash_single_entry(
        root_path: &Path,
        entry: &FileEntry,
        visited: &mut HashSet<PathBuf>,
    ) -> io::Result<String> {
        let mut hasher = blake3::Hasher::new();

        let rel_path_str = entry.rel_path.to_string_lossy().replace('\\', "/");
        hasher.update(rel_path_str.as_bytes());

        match (
            entry.file_type.is_file(),
            entry.file_type.is_dir(),
            entry.file_type.is_symlink(),
        ) {
            // 普通文件情况
            (true, false, false) => {
                let full_path = root_path.join(&entry.rel_path);
                let mut file = File::open(full_path)?;

                let mut buffer = [0u8; 8192];
                loop {
                    let count = file.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    hasher.update(&buffer[..count]);
                }
            }
            // 目录情况
            (false, true, false) => {
                hasher.update(b"[DIR]");
            }
            // 软链接情况：延迟 Follow 真实数据
            (false, false, true) => {
                let full_path = root_path.join(&entry.rel_path);

                // 使用 canonicalize 追踪到最终的真实绝对路径（能自动处理多层嵌套软链接）
                let canonical_path = fs::canonicalize(&full_path)?;

                // 获取真实目标的元数据（fs::metadata 会自动穿透软链接）
                let target_metadata = fs::metadata(&canonical_path)?;

                if target_metadata.is_file() {
                    let mut file = File::open(&canonical_path)?;
                    let mut buffer = [0u8; 8192];
                    loop {
                        let count = file.read(&mut buffer)?;
                        if count == 0 {
                            break;
                        }
                        hasher.update(&buffer[..count]);
                    }
                } else if target_metadata.is_dir() {
                    // 环路检测：如果该真实路径已经在当前的递归祖先链中，说明存在循环引用，抛出错误
                    if visited.contains(&canonical_path) {
                        return Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            format!("Circular symlink detected at {:?}", full_path),
                        ));
                    }

                    // 【入栈】：记录当前正在向下递归的真实目录
                    visited.insert(canonical_path.clone());

                    // 递归为该目标目录创建一棵新的 FileMerkleTree，并计算其 Merkle Root Hash
                    let mut sub_tree = FileMerkleTree::new(canonical_path.clone())?;
                    let sub_hash = sub_tree.get_flat_hash_internal(visited)?;

                    // 【出栈/回溯】：计算完成，将其从当前链中移出
                    // 这样可以允许并列的其他软链接也指向这个目录（即支持 DAG 依赖结构）
                    visited.remove(&canonical_path);

                    // 将子树的根哈希作为该软链接的“内容凭证”
                    hasher.update(sub_hash.as_bytes());
                } else {
                    println!(
                        "Warning: Symlink points to an unsupported file type at {:?}",
                        canonical_path
                    );
                    hasher.update(b"[UNKNOWN_TARGET]");
                }
            }
            // 其他未知类型（如：管道、块设备、字符设备等），打印日志并跳过
            _ => {
                println!(
                    "Warning: Skipped unknown file type for entry: {:?}",
                    entry.rel_path
                );
            }
        }

        Ok(hasher.finalize().to_hex().to_string())
    }

    fn get_flat_hash(&mut self) -> io::Result<String> {
        let mut visited = HashSet::new();

        // 初始化：将当前树的根目录的真实绝对路径放入已访问集合（作为防环起点）
        if let Ok(canonical_root) = fs::canonicalize(&**self.root_path) {
            visited.insert(canonical_root);
        }

        self.get_flat_hash_internal(&mut visited)
    }

    /// 内部递归调用的计算方法，传递并共享 visited 集合状态
    fn get_flat_hash_internal(&mut self, visited: &mut HashSet<PathBuf>) -> io::Result<String> {
        if self.entries.is_empty() {
            return Ok(blake3::hash(b"empty_tree").to_hex().to_string());
        }

        self.entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

        let root_path = Arc::clone(&self.root_path);

        for entry in self.entries.iter_mut() {
            entry.hash = Self::hash_single_entry(&root_path, entry, visited)?;
        }

        let mut current_layer: Vec<String> = self.entries.iter().map(|e| e.hash.clone()).collect();

        while current_layer.len() > 1 {
            let mut next_layer = Vec::new();

            for chunk in current_layer.chunks(2) {
                let mut hasher = blake3::Hasher::new();
                if chunk.len() == 2 {
                    hasher.update(chunk[0].as_bytes());
                    hasher.update(chunk[1].as_bytes());
                } else {
                    hasher.update(chunk[0].as_bytes());
                    hasher.update(chunk[0].as_bytes());
                }
                next_layer.push(hasher.finalize().to_hex().to_string());
            }
            current_layer = next_layer;
        }

        Ok(current_layer.into_iter().next().unwrap())
    }

    /// 核心重组逻辑：基于深度的自底向上聚合
    fn rearange_into_file_tree_form(&mut self) -> io::Result<String> {
        if self.entries.is_empty() {
            return Ok(blake3::hash(b"empty_tree").to_hex().to_string());
        }

        // 1. 按路径深度降序排序（最深的文件/文件夹排在最前面，Root 节点最后处理）
        self.entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.rel_path.components().count()));

        // 2. 准备所有目录的“收件箱” (Inbox)
        // Key: 父目录的 rel_path, Value: 子项列表 (节点名称, 是否为目录, 哈希值)
        let mut inbox: HashMap<PathBuf, Vec<(String, bool, String)>> = HashMap::new();
        let mut final_root_hash = String::new();

        for entry in self.entries.iter_mut() {
            // WalkDir 遍历的根目录在 strip_prefix 后为空路径 ""
            let is_root = entry.rel_path.as_os_str().is_empty();

            let name = if is_root {
                String::new()
            } else {
                entry
                    .rel_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };

            let parent_path = entry
                .rel_path
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf();

            // 如果当前条目是目录，说明比它更深的子项都已经遍历过了，并且把数据投递到了它的收件箱中
            if entry.file_type.is_dir() {
                let mut children = inbox.remove(&entry.rel_path).unwrap_or_default();

                // N-ary 多叉树核心规则：子项必须按名称字典序严格排序，确保哈希的绝对确定性
                children.sort_by(|a, b| a.0.cmp(&b.0));

                let mut hasher = blake3::Hasher::new();
                for child in children {
                    hasher.update(child.0.as_bytes()); // 绑定子项名称
                    hasher.update(child.2.as_bytes()); // 绑定子项哈希
                    hasher.update(if child.1 { &[1] } else { &[0] }); // 绑定类型标识
                }

                // 将重新计算出的真实多叉树哈希，覆盖掉原本 get_flat_hash 留下的占位符
                entry.hash = hasher.finalize().to_hex().to_string();
            }

            if is_root {
                final_root_hash = entry.hash.clone();
            } else {
                // 将自己（文件或算完哈希的子目录）投递给上级目录的收件箱
                inbox.entry(parent_path).or_default().push((
                    name,
                    entry.file_type.is_dir(),
                    entry.hash.clone(),
                ));
            }
        }

        // 3. 兜底逻辑：在某些特殊遍历行为下，WalkDir 可能不显式 yield 根目录本身。
        // 如果 final_root_hash 仍为空，说明树的第一层文件都在 key 为 "" 的收件箱里。
        if final_root_hash.is_empty() {
            let mut children = inbox.remove(Path::new("")).unwrap_or_default();
            children.sort_by(|a, b| a.0.cmp(&b.0));

            let mut hasher = blake3::Hasher::new();
            for child in children {
                hasher.update(child.0.as_bytes());
                hasher.update(child.2.as_bytes());
                hasher.update(if child.1 { &[1] } else { &[0] });
            }
            final_root_hash = hasher.finalize().to_hex().to_string();
        }

        Ok(final_root_hash)
    }

    /// 外部调用的公共方法
    pub fn get_hash(&mut self) -> io::Result<String> {
        // 第一阶段：Map 阶段
        // 计算所有叶子节点的基础哈希。此方法底层顺便做了一次废弃的二叉树归并，
        // 返回的根哈希并不符合我们的 N-ary 结构要求，所以直接用 `_` 忽略掉它。
        // 我们需要的是它执行完毕后，self.entries 里填满的叶子哈希。
        let _ = self.get_flat_hash()?;

        // 第二阶段：Reduce 阶段
        // 在内存中重塑多叉树层级，重算内部文件夹的哈希，并返回真实的 Merkle Root
        self.rearange_into_file_tree_form()
    }
}
