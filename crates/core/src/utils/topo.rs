use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use crate::EdgelinkError;

#[derive(Clone)]
struct Vertex<Item> {
    item: Item,
    in_degree: usize,
}

pub struct TopologicalSorter<Item> {
    vertices: HashMap<Item, Vertex<Item>>,
    edges: HashMap<Item, HashSet<Item>>,
}

impl<Item> Default for TopologicalSorter<Item>
where
    Item: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Item> TopologicalSorter<Item>
where
    Item: Clone + Eq + Hash,
{
    pub fn new() -> Self {
        TopologicalSorter { vertices: HashMap::new(), edges: HashMap::new() }
    }

    pub fn add_vertex(&mut self, item: Item) {
        if !self.vertices.contains_key(&item) {
            self.vertices.insert(item.clone(), Vertex { item: item.clone(), in_degree: 0 });
        }
    }

    pub fn add_dep(&mut self, from: Item, to: Item) {
        self.vertices.entry(from.clone()).or_insert(Vertex { item: from.clone(), in_degree: 0 });
        let to_vertex = self.vertices.entry(to.clone()).or_insert(Vertex { item: to.clone(), in_degree: 0 });
        self.edges.entry(from.clone()).or_default().insert(to.clone());
        to_vertex.in_degree += 1;
    }

    pub fn add_deps(&mut self, from: Item, tos: impl IntoIterator<Item = Item>) {
        for to in tos {
            self.vertices.entry(from.clone()).or_insert(Vertex { item: from.clone(), in_degree: 0 });
            let to_vertex = self.vertices.entry(to.clone()).or_insert(Vertex { item: to.clone(), in_degree: 0 });
            self.edges.entry(from.clone()).or_default().insert(to.clone());
            to_vertex.in_degree += 1;
        }
    }

    pub fn topological_sort(&self) -> crate::Result<Vec<Item>> {
        let mut in_degree = self.vertices.values().map(|v| (v.item.clone(), v.in_degree)).collect::<HashMap<_, _>>();

        let mut sorted = Vec::with_capacity(self.vertices.len());
        let mut sources: Vec<Item> =
            in_degree.iter().filter(|&(_, &degree)| degree == 0).map(|(item, _)| item.clone()).collect();

        while let Some(source) = sources.pop() {
            sorted.push(source.clone());

            if let Some(neighbors) = self.edges.get(&source) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            sources.push(neighbor.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles
        if sorted.len() != self.vertices.len() {
            return Err(EdgelinkError::InvalidOperation("Graph has cycles".to_string()).into());
        }

        Ok(sorted)
    }

    pub fn dependency_sort(&self) -> crate::Result<Vec<Item>> {
        let mut result = self.topological_sort()?;
        result.reverse();
        Ok(result)
    }
}

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn test_simple_linear_dependency() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "B");
        graph.add_dep("B", "C");

        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_multiple_sources() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "C");
        graph.add_dep("B", "C");

        let sorted = graph.topological_sort().unwrap();
        assert!(sorted == vec!["A", "B", "C"] || sorted == vec!["B", "A", "C"]);
    }

    #[test]
    fn test_complex_dependency() {
        let mut graph = TopologicalSorter::new();
        graph.add_deps("A", ["B", "C"]);
        graph.add_dep("B", "D");
        graph.add_dep("C", "D");
        graph.add_dep("D", "E");

        let sorted = graph.topological_sort().unwrap();
        assert!(sorted.contains(&"A"));
        assert!(sorted.contains(&"B"));
        assert!(sorted.contains(&"C"));
        assert!(sorted.contains(&"D"));
        assert!(sorted.contains(&"E"));

        let a_index = sorted.iter().position(|&x| x == "A").unwrap();
        let b_index = sorted.iter().position(|&x| x == "B").unwrap();
        let c_index = sorted.iter().position(|&x| x == "C").unwrap();
        let d_index = sorted.iter().position(|&x| x == "D").unwrap();
        let e_index = sorted.iter().position(|&x| x == "E").unwrap();

        assert!(a_index < b_index);
        assert!(a_index < c_index);
        assert!(b_index < d_index);
        assert!(c_index < d_index);
        assert!(d_index < e_index);
    }

    #[test]
    fn test_multiple_layers() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "C");
        graph.add_dep("B", "C");
        graph.add_dep("C", "D");
        graph.add_dep("D", "E");
        graph.add_dep("E", "F");

        let sorted = graph.topological_sort().unwrap();
        assert!(sorted == vec!["A", "B", "C", "D", "E", "F"] || sorted == vec!["B", "A", "C", "D", "E", "F"]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "B");
        graph.add_dep("B", "C");
        graph.add_dep("C", "A");

        let result = std::panic::catch_unwind(|| {
            graph.topological_sort().unwrap();
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_cycles() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "B");
        graph.add_dep("B", "C");
        graph.add_dep("C", "A");
        graph.add_dep("D", "E");
        graph.add_dep("E", "D");

        let result = std::panic::catch_unwind(|| {
            graph.topological_sort().unwrap();
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_independent_nodes() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "B");
        graph.add_dep("C", "D");

        let sorted = graph.topological_sort().unwrap();
        assert!(
            sorted == vec!["A", "B", "C", "D"]
                || sorted == vec!["A", "C", "B", "D"]
                || sorted == vec!["C", "D", "A", "B"]
        );
    }

    #[test]
    fn test_large_graph() {
        let mut graph = TopologicalSorter::new();
        for i in 0..100 {
            for j in (i + 1)..100 {
                graph.add_dep(i.to_string(), j.to_string());
            }
        }

        let sorted = graph.topological_sort().unwrap();
        for i in 0..100 {
            for j in (i + 1)..100 {
                assert!(
                    sorted.iter().position(|x| x == &i.to_string()) < sorted.iter().position(|x| x == &j.to_string())
                );
            }
        }
    }

    #[test]
    fn test_single_node() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "A");

        let result = std::panic::catch_unwind(|| {
            graph.topological_sort().unwrap();
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_no_dependencies() {
        let mut graph = TopologicalSorter::new();
        graph.add_dep("A", "B");
        graph.add_dep("C", "D");
        graph.add_dep("E", "F");

        let sorted = graph.topological_sort().unwrap();
        assert!(sorted.len() == 6);
        assert!(sorted.contains(&"A"));
        assert!(sorted.contains(&"B"));
        assert!(sorted.contains(&"C"));
        assert!(sorted.contains(&"D"));
        assert!(sorted.contains(&"E"));
        assert!(sorted.contains(&"F"));
    }

    #[test]
    fn test_dependency_sort() {
        let mut graph = TopologicalSorter::new();
        graph.add_deps("A", ["B", "C"]);
        graph.add_dep("B", "D");
        graph.add_dep("C", "D");
        graph.add_dep("D", "E");
        graph.add_vertex("F");

        let sorted = graph.dependency_sort().unwrap();
        assert_eq!(sorted.len(), 6);
        assert!(sorted.contains(&"A"));
        assert!(sorted.contains(&"B"));
        assert!(sorted.contains(&"C"));
        assert!(sorted.contains(&"D"));
        assert!(sorted.contains(&"E"));
        assert!(sorted.contains(&"F"));

        let a_index = sorted.iter().position(|&x| x == "A").unwrap();
        let b_index = sorted.iter().position(|&x| x == "B").unwrap();
        let c_index = sorted.iter().position(|&x| x == "C").unwrap();
        let d_index = sorted.iter().position(|&x| x == "D").unwrap();
        let e_index = sorted.iter().position(|&x| x == "E").unwrap();
        let f_index = sorted.iter().position(|&x| x == "F").unwrap();

        assert!(f_index == 0 || f_index == 5);
        assert!(b_index < a_index);
        assert!(c_index < a_index);
        assert!(d_index < b_index);
        assert!(d_index < c_index);
        assert!(d_index < a_index);
        assert!(e_index < d_index);
        assert!(e_index < b_index);
        assert!(e_index < c_index);
        assert!(e_index < a_index);
    }
}
