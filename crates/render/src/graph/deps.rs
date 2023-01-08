use pulz_bitset::{BitSet, BitSetIter};

pub struct DependencyMatrix {
    num_dependencies: Vec<usize>,
    num_dependents: Vec<usize>,
    matrix: BitSet,
}

impl DependencyMatrix {
    pub fn new(num_nodes: usize) -> Self {
        let mut num_dependencies = Vec::new();
        let mut num_dependents = Vec::new();
        num_dependencies.resize(num_nodes, 0);
        num_dependents.resize(num_nodes, 0);
        let deps = BitSet::with_capacity_for(num_nodes * num_nodes);
        Self {
            num_dependencies,
            num_dependents,
            matrix: deps,
        }
    }

    #[inline]
    pub fn num_nodes(&self) -> usize {
        self.num_dependencies.len()
    }

    #[inline]
    pub fn num_dependencies(&self, index: usize) -> usize {
        self.num_dependencies.get(index).copied().unwrap_or(0)
    }

    #[inline]
    pub fn num_dependents(&self, index: usize) -> usize {
        self.num_dependencies.get(index).copied().unwrap_or(0)
    }

    pub fn dependents(&self, from: usize) -> BitSetIter<'_> {
        let len = self.num_nodes();
        let start_index = len * from;
        self.matrix.iter_range(start_index..start_index + len)
    }

    #[inline]
    fn index(&self, from: usize, to: usize) -> usize {
        self.num_nodes() * from + to
    }

    pub fn insert(&mut self, from: usize, to: usize) -> bool {
        if self.matrix.insert(self.index(from, to)) {
            self.num_dependents[from] += 1;
            self.num_dependencies[to] += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, from: usize, to: usize) -> bool {
        self.matrix.contains(self.index(from, to))
    }

    pub fn remove(&mut self, from: usize, to: usize) -> bool {
        if self.matrix.remove(self.index(from, to)) {
            self.num_dependents[from] -= 1;
            self.num_dependencies[to] -= 1;
            true
        } else {
            false
        }
    }

    pub fn remove_self_references(&mut self) {
        for i in 0..self.num_nodes() {
            self.remove(i, i);
        }
    }

    pub fn remove_from_dependents(&mut self, from: usize) {
        for _i in 0..self.num_nodes() {
            let len = self.num_nodes();
            let start_index = len * from;
            for d in self.matrix.drain(start_index..start_index + len) {
                self.num_dependents[d] -= 1;
            }
            self.num_dependents[from] = 0;
        }
    }

    pub fn clear(&mut self) {
        self.matrix.clear();
        for e in &mut self.num_dependencies {
            *e = 0;
        }
        for e in &mut self.num_dependents {
            *e = 0;
        }
    }

    pub fn into_topological_order(mut self) -> Vec<Vec<usize>> {
        let mut todo = BitSet::from_range(0..self.num_nodes());
        let mut result = Vec::new();
        loop {
            let mut new_nodes = Vec::new();
            let mut total_deps = 0;
            for i in &todo {
                let num_deps = self.num_dependencies[i];
                total_deps += num_deps;
                if num_deps == 0 {
                    new_nodes.push(i);
                }
            }

            if new_nodes.is_empty() {
                assert_eq!(0, total_deps, "cycle detected");
                break;
            }

            for i in new_nodes.iter().copied() {
                todo.remove(i);
                self.remove_from_dependents(i);
            }

            result.push(new_nodes);
        }
        result
    }
}
