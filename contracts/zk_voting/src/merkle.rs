use soroban_sdk::{BytesN, Env, Vec};

pub type NodeHash = BytesN<32>;

fn hash_pair(env: &Env, left: &NodeHash, right: &NodeHash) -> NodeHash {
    let mut combined = soroban_sdk::Bytes::new(env);
    combined.extend_from_array(&left.to_array());
    combined.extend_from_array(&right.to_array());
    env.crypto().keccak256(&combined).into()
}

fn next_power_of_two(n: u32) -> u32 {
    if n <= 1 {
        return 1;
    }
    let mut p = 1u32;
    while p < n {
        p <<= 1;
    }
    p
}

pub struct MerkleTree {
    nodes: Vec<NodeHash>,
    pub leaf_count: u32,
}

impl MerkleTree {
    pub fn new(env: &Env, leaves: Vec<NodeHash>) -> Self {
        assert!(!leaves.is_empty(), "MerkleTree: need at least one leaf");

        let n = next_power_of_two(leaves.len());
        let mut padded: Vec<NodeHash> = Vec::new(env);
        for i in 0..leaves.len() {
            padded.push_back(leaves.get(i).unwrap());
        }
        let last = leaves.get(leaves.len() - 1).unwrap();
        for _ in leaves.len()..n {
            padded.push_back(last.clone());
        }

        let mut all_nodes: Vec<NodeHash> = Vec::new(env);
        for i in 0..padded.len() {
            all_nodes.push_back(padded.get(i).unwrap());
        }

        let mut level_size = n;
        let mut level_start: u32 = 0;
        while level_size > 1 {
            let next_size = level_size / 2;
            for i in 0..next_size {
                let left = all_nodes.get(level_start + i * 2).unwrap();
                let right = all_nodes.get(level_start + i * 2 + 1).unwrap();
                all_nodes.push_back(hash_pair(env, &left, &right));
            }
            level_start += level_size;
            level_size = next_size;
        }

        MerkleTree {
            nodes: all_nodes,
            leaf_count: n,
        }
    }

    pub fn root(&self) -> NodeHash {
        self.nodes.get(self.nodes.len() - 1).unwrap()
    }

    pub fn leaf(&self, index: u32) -> NodeHash {
        self.nodes.get(index).unwrap()
    }

    pub fn proof(&self, env: &Env, index: u32) -> Vec<NodeHash> {
        let mut siblings: Vec<NodeHash> = Vec::new(env);
        let mut idx = index;
        let mut level_start: u32 = 0;
        let mut level_size = self.leaf_count;
        while level_size > 1 {
            let sibling = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            siblings.push_back(self.nodes.get(level_start + sibling).unwrap());
            level_start += level_size;
            level_size /= 2;
            idx /= 2;
        }
        siblings
    }

    pub fn verify_proof(
        env: &Env,
        root: &NodeHash,
        leaf: &NodeHash,
        index: u32,
        siblings: &Vec<NodeHash>,
    ) -> bool {
        let mut current = leaf.clone();
        let mut idx = index;
        for i in 0..siblings.len() {
            let sibling = siblings.get(i).unwrap();
            current = if idx % 2 == 0 {
                hash_pair(env, &current, &sibling)
            } else {
                hash_pair(env, &sibling, &current)
            };
            idx /= 2;
        }
        &current == root
    }
}

pub fn make_leaf(env: &Env, seed: u8) -> NodeHash {
    let mut raw = [0u8; 32];
    raw[0] = seed;
    let b = soroban_sdk::Bytes::from_array(env, &raw);
    env.crypto().keccak256(&b).into()
}
