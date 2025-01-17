use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Graph;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::BufRead;
use std::{fs, io};
use strum::EnumCount;
use strum_macros::{EnumCount, EnumIter};

// Seed for generating everything
const SEED: &str = "Generate";

// Number of towns to generate
const NUM_OF_TOWNS: usize = 15;

// Number of connections between towns
const NUM_OF_CONNECTIONS: u32 = 23;

// Min distance between towns
const MIN_DISTANCE: u32 = 2;

// Max distance between towns
const MAX_DISTANCE: u32 = 50;

// Initial cost of travel which will be mutiplied by distance
const COST: u32 = 5;

// Max number for IDs
const MAX_ID: u32 = 100000;

// Min number of buildings per town
const MIN_BUILDINGS: u32 = 5;

// Max number of buildings per town
const MAX_BUILDINGS: u32 = 25;

// Min number of NPCs per building
const MIN_NPCS: u32 = 4;

// Max number of NPCs per building
const MAX_NPCS: u32 = 10;

// Input directory for loading files
const INPUT_DIR: &str = "input";

// Output directory for saving files
const OUTPUT_DIR: &str = "output";

// Struct for representing a town
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Town {
    id: u32,
    name: String,
    number_of_buildings: u32,
    buildings: Vec<Building>,
}

// Struct for representing a raw town (containing just a town's name) when importing a DOT file.
#[derive(Debug, Clone)]
struct TownRaw {
    name: String,
}

// Struct for storing distance between towns and cost in the edges
#[derive(Debug, Clone)]
struct JourneyInfo {
    distance: u32,
    cost: u32,
}

impl JourneyInfo {
    fn from_label(label: &str) -> Option<Self> {
        let parts: Vec<&str> = label.split("/").map(|s| s.trim()).collect();
        if parts.len() == 2 {
            let distance = parts[0].trim().split_whitespace().next()?.parse().ok()?;
            let cost = parts[1].trim().split_whitespace().next()?.parse().ok()?;
            Some(Self { distance, cost })
        } else {
            None
        }
    }
}

// Struct for representing a building
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Building {
    id: u32,
    name: String,
    building_type: BuildingType,
    doors: u32,
    windows: u32,
    coords: (u32, u32),
    npcs: Vec<Npc>,
}

// Enum for building types
#[derive(Serialize, Deserialize, Debug, Clone, EnumCount, EnumIter)]
enum BuildingType {
    Residence,
    Shop,
    Tavern,
    Temple,
}

// Struct for representing an NPC
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Npc {
    id: u32,
    name: String,
    sex: NpcSex,
    race: NpcRace,
}

// Enum for NPC sex
#[derive(Serialize, Deserialize, Debug, Clone, EnumCount, EnumIter)]
enum NpcSex {
    Male,
    Female,
    Unisex,
}

// Enum for NPC race
#[derive(Serialize, Deserialize, Debug, Clone, EnumCount, EnumIter)]
enum NpcRace {
    Human,
    Elf,
}

// Function to generate multiple towns and create a graph
fn generate_towns(number_of_towns: usize) -> (Graph<Town, JourneyInfo>, Vec<Town>) {
    let seed = seed_from_word();
    let mut rng = StdRng::seed_from_u64(seed); //Creates an StdRng (seedable random number generator) using the seed.

    let mut town_ids = HashSet::new();
    let mut building_ids = HashSet::new();
    let mut npc_ids = HashSet::new();

    let mut towns = Vec::new();

    for _ in 0..number_of_towns {
        let mut town_id = rng.gen_range(1..MAX_ID);
        while town_ids.contains(&town_id) {
            town_id = rng.gen_range(1..MAX_ID);
        }
        town_ids.insert(town_id);

        let number_of_buildings = rng.gen_range(MIN_BUILDINGS..MAX_BUILDINGS);
        let buildings = generate_buildings(
            &mut rng,
            number_of_buildings,
            &mut building_ids,
            &mut npc_ids,
        );

        towns.push(Town {
            id: town_id,
            name: generate_town_name(&mut rng),
            number_of_buildings,
            buildings,
        });
    }

    let graph_and_nodes: (Graph<Town, JourneyInfo>, Vec<NodeIndex>) =
        generate_graph(&mut rng, towns);

    display_graph(&graph_and_nodes.0, &graph_and_nodes.1);

    let list_of_towns: Vec<Town> = graph_and_nodes
        .1
        .iter()
        .map(|&node| graph_and_nodes.0[node].clone())
        .collect();

    (graph_and_nodes.0, list_of_towns)
}

// Function to derive a consistent seed from a word or phrase
fn seed_from_word() -> u64 {
    let mut hasher = DefaultHasher::new();
    SEED.hash(&mut hasher);
    hasher.finish()
}

// Function to generate buildings
fn generate_buildings(
    rng: &mut StdRng,
    number_of_buildings: u32,
    building_ids: &mut HashSet<u32>,
    npc_ids: &mut HashSet<u32>,
) -> Vec<Building> {
    let mut buildings = Vec::new();
    let grid_size = (number_of_buildings as f32).sqrt().ceil() as u32;
    let mut position = (0, 0);

    for _ in 0..number_of_buildings {
        let mut building_id = rng.gen_range(1..MAX_ID);
        while building_ids.contains(&building_id) {
            building_id = rng.gen_range(1..MAX_ID);
        }
        building_ids.insert(building_id);

        let building_type = match rng.gen_range(0..BuildingType::COUNT) {
            0 => BuildingType::Residence,
            1 => BuildingType::Shop,
            2 => BuildingType::Tavern,
            3 => BuildingType::Temple,
            _ => BuildingType::Residence,
        };

        let doors = rng.gen_range(1..=3);
        let windows = rng.gen_range(2..10);

        let mut building = Building {
            id: building_id,
            name: generate_building_name(rng, &building_type),
            building_type,
            doors,
            windows,
            coords: position,
            npcs: Vec::new(),
        };

        building.npcs = generate_npcs(rng, &building.building_type, &building.name, npc_ids);

        buildings.push(building);

        position.0 += 1;
        if position.0 >= grid_size {
            position.0 = 0;
            position.1 += 1;
        }
    }

    buildings
}

// Generate a building name
fn generate_building_name(rng: &mut StdRng, building_type: &BuildingType) -> String {
    let surnames = load_list("surnames.txt");
    let shops = load_list("shops.txt");
    let taverns = load_list("taverns.txt");
    let temples = load_list("temples.txt");

    match building_type {
        BuildingType::Residence => {
            if let Some(name) = surnames.get(rng.gen_range(0..surnames.len())) {
                format!("{} Residence", name)
            } else {
                "NO DATA".into()
            }
        }
        BuildingType::Shop => {
            if let Some(name) = surnames.get(rng.gen_range(0..surnames.len())) {
                if let Some(shop) = shops.get(rng.gen_range(0..shops.len())) {
                    format!("{}'s {}", name, shop)
                } else {
                    "NO DATA".into()
                }
            } else {
                "NO DATA".into()
            }
        }
        BuildingType::Tavern => {
            if let Some(name) = taverns.get(rng.gen_range(0..taverns.len())) {
                name.to_string()
            } else {
                "NO DATA".into()
            }
        }
        BuildingType::Temple => {
            if let Some(temple) = temples.get(rng.gen_range(0..temples.len())) {
                format!("Temple of the {}", temple)
            } else {
                "NO DATA".into()
            }
        }
    }
}

// Generate NPCs
fn generate_npcs(
    rng: &mut StdRng,
    building_type: &BuildingType,
    building_name: &str,
    npc_ids: &mut HashSet<u32>,
) -> Vec<Npc> {
    let mut npcs = Vec::new();

    let number_of_npcs = match building_type {
        BuildingType::Shop => 1,
        BuildingType::Residence => 2,
        _ => rng.gen_range(MIN_NPCS..MAX_NPCS),
    };

    for _ in 0..number_of_npcs {
        let mut npc_id = rng.gen_range(1..MAX_ID);
        while npc_ids.contains(&npc_id) {
            npc_id = rng.gen_range(1..MAX_ID);
        }
        npc_ids.insert(npc_id);

        let sex = match rng.gen_range(0..NpcSex::COUNT) {
            0 => NpcSex::Male,
            1 => NpcSex::Female,
            2 => NpcSex::Unisex,
            _ => NpcSex::Unisex,
        };

        let race = match rng.gen_range(0..NpcRace::COUNT) {
            0 => NpcRace::Human,
            1 => NpcRace::Elf,
            _ => NpcRace::Human,
        };

        npcs.push(Npc {
            id: npc_id,
            name: generate_npc_name(rng, building_type, building_name, &sex),
            sex,
            race,
        });
    }

    npcs
}

// Generate an NPC name
fn generate_npc_name(
    rng: &mut StdRng,
    building_type: &BuildingType,
    building_name: &str,
    npc_sex: &NpcSex,
) -> String {
    let names_male = load_list("names-male.txt");
    let names_female = load_list("names-female.txt");
    let names_unisex = load_list("names-unisex.txt");
    let surnames = load_list("surnames.txt");

    let firstname: String;

    match npc_sex {
        NpcSex::Male => {
            if let Some(name) = names_male.get(rng.gen_range(0..names_male.len())) {
                firstname = name.to_string();
            } else {
                firstname = "NO DATA".into();
            }
        }
        NpcSex::Female => {
            if let Some(name) = names_female.get(rng.gen_range(0..names_female.len())) {
                firstname = name.to_string();
            } else {
                firstname = "NO DATA".into();
            }
        }
        NpcSex::Unisex => {
            if let Some(name) = names_unisex.get(rng.gen_range(0..names_unisex.len())) {
                firstname = name.to_string();
            } else {
                firstname = "NO DATA".into();
            }
        }
    };

    match building_type {
        BuildingType::Residence => {
            let surname = building_name.split([' ', '\'']).next().unwrap();
            format!("{} {}", firstname, surname)
        }
        BuildingType::Shop => {
            let surname = building_name.split([' ', '\'']).next().unwrap();
            format!("{} {}", firstname, surname)
        }
        BuildingType::Tavern => {
            if let Some(surname) = surnames.get(rng.gen_range(0..surnames.len())) {
                format!("{} {}", firstname, surname)
            } else {
                format!("{} NO DATA", firstname)
            }
        }
        BuildingType::Temple => {
            format!("{} of the {}", firstname, building_name)
        }
    }
}

// Generate a town name using a prefix-root-suffix combination
fn generate_town_name(rng: &mut StdRng) -> String {
    let prefixes = load_list("town-prefixes.txt");
    let roots = load_list("town-roots.txt");
    let suffixes = load_list("town-suffixes.txt");

    let prefix: String;
    if let Some(name) = prefixes.get(rng.gen_range(0..prefixes.len())) {
        prefix = name.to_string();
    } else {
        prefix = "NO DATA".into();
    }

    let root: String;
    if let Some(name) = roots.get(rng.gen_range(0..roots.len())) {
        root = name.to_string();
    } else {
        root = "NO DATA".into();
    }

    let suffix: String;
    if let Some(name) = suffixes.get(rng.gen_range(0..suffixes.len())) {
        suffix = name.to_string();
    } else {
        suffix = "NO DATA".into();
    }

    format!("{} {}{}", prefix, root, suffix)
}

// Generate a graph with towns and edges with distance and cost
fn generate_graph(
    rng: &mut StdRng,
    towns: Vec<Town>,
) -> (Graph<Town, JourneyInfo>, Vec<NodeIndex>) {
    let mut town_graph = Graph::<Town, JourneyInfo>::new();
    let mut town_nodes = Vec::new();

    for town in towns {
        let node = town_graph.add_node(town);
        town_nodes.push(node);
    }

    for _ in 0..NUM_OF_CONNECTIONS {
        let town1 = town_nodes[rng.gen_range(0..town_nodes.len())];
        let town2 = town_nodes[rng.gen_range(0..town_nodes.len())];

        if town1 != town2 && !town_graph.contains_edge(town1, town2) {
            let distance = rng.gen_range(MIN_DISTANCE..MAX_DISTANCE);
            let cost = COST * distance;

            town_graph.add_edge(town1, town2, JourneyInfo { distance, cost });
        }
    }

    (town_graph, town_nodes)
}

// Display towns and their connections with distance and cost
fn display_graph(graph: &Graph<Town, JourneyInfo>, nodes: &Vec<NodeIndex>) {
    for node in nodes {
        let town = &graph[*node];
        println!(
            "Town: {} (Buildings: {})",
            town.name, town.number_of_buildings
        );

        for edge in graph.edges(*node) {
            let target_town = &graph[edge.target()];
            let journey_info = edge.weight();
            println!(
                " -> {}: Distance = {} miles, Cost = {} gold",
                target_town.name, journey_info.distance, journey_info.cost
            );
        }
    }
}

// Save graph to a DOT file
fn save_graph(graph: &Graph<Town, JourneyInfo>, filename: &str) -> Result<String, std::io::Error> {
    let mut dot_output = String::from("graph Towns {\n");

    for edge in graph.edge_references() {
        let source_town = &graph[edge.source()];
        let target_town = &graph[edge.target()];
        let journey_info = edge.weight();

        dot_output.push_str(&format!(
            "    \"{}\" -- \"{}\" [label=\"{} m / {} gold\", len={}];\n",
            source_town.name,
            target_town.name,
            journey_info.distance,
            journey_info.cost,
            journey_info.distance / 10
        ));
    }

    dot_output.push_str("}\n");

    let filepath = format!("{}/{}", OUTPUT_DIR, filename);
    fs::create_dir_all(OUTPUT_DIR)?;
    fs::write(filepath, dot_output)?;

    Ok("Graph DOT file saved successfully".into())
}

// Save towns to a JSON file
fn save_towns(towns: &Vec<Town>, filename: &str) -> Result<String, std::io::Error> {
    let json = serde_json::to_string_pretty(towns)?;

    let filepath = format!("{}/{}", OUTPUT_DIR, filename);
    fs::create_dir_all(OUTPUT_DIR)?;
    fs::write(filepath, json)?;

    Ok("Towns JSON file saved successfully".into())
}

// Import a DOT file and generate Towns and Graph
fn import(filename: &str) -> Result<(Graph<Town, JourneyInfo>, Vec<Town>), std::io::Error> {
    let imported_raw_graph = load_dot(filename)?;

    let imported_towns = generate_towns_from_imported_raw_graph(&imported_raw_graph);
    let imported_graph = generate_graph_from_imported_towns(&imported_raw_graph, &imported_towns);

    Ok((imported_graph, imported_towns))
}

// Load a DOT file
fn load_dot(filename: &str) -> Result<Graph<TownRaw, JourneyInfo>, std::io::Error> {
    let file_content = fs::read_to_string(filename)?;

    let mut graph = Graph::<TownRaw, JourneyInfo>::new();

    let mut node_indices = HashMap::new();

    for line in file_content.lines() {
        if let Some((source, target, label)) = parse_edge_line(line) {
            let src_index = *node_indices.entry(source.clone()).or_insert_with(|| {
                graph.add_node(TownRaw {
                    name: source.clone(),
                })
            });
            let tgt_index = *node_indices.entry(target.clone()).or_insert_with(|| {
                graph.add_node(TownRaw {
                    name: target.clone(),
                })
            });

            if let Some(journey_info) = JourneyInfo::from_label(&label) {
                graph.add_edge(src_index, tgt_index, journey_info);
            }
        }
    }

    Ok(graph)
}

// Parse a DOT file
fn parse_edge_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();

    if line.starts_with('"') && line.contains("--") && line.contains("[label=") {
        let parts: Vec<&str> = line.split("--").collect();
        if parts.len() == 2 {
            // Trim and remove quotes from the source town
            let source = parts[0].trim().trim_matches('"').to_string();

            let target_and_label = parts[1].trim();

            // Find where the target ends (right before the `[`)
            let target_end = target_and_label.find('[')?;
            let target = target_and_label[..target_end]
                .trim()
                .trim_matches('"')
                .to_string();

            // Extract the label content within quotes
            let label_start = target_and_label.find("label=\"")? + 7;
            let label_end = target_and_label[label_start..].find('"')? + label_start;
            let label = target_and_label[label_start..label_end].trim().to_string();

            return Some((source, target, label));
        }
    }
    None
}

// Generate towns from a loaded in DOT file
fn generate_towns_from_imported_raw_graph(graph: &Graph<TownRaw, JourneyInfo>) -> Vec<Town> {
    let town_names: Vec<String> = graph.node_weights().map(|town| town.name.clone()).collect();

    let seed = seed_from_word();
    let mut rng = StdRng::seed_from_u64(seed); //Creates an StdRng (seedable random number generator) using the seed.

    let mut town_ids = HashSet::new();
    let mut building_ids = HashSet::new();
    let mut npc_ids = HashSet::new();

    let mut towns = Vec::new();

    for townname in town_names {
        let mut town_id = rng.gen_range(1..MAX_ID);
        while town_ids.contains(&town_id) {
            town_id = rng.gen_range(1..MAX_ID);
        }
        town_ids.insert(town_id);

        let number_of_buildings = rng.gen_range(MIN_BUILDINGS..MAX_BUILDINGS);
        let buildings = generate_buildings(
            &mut rng,
            number_of_buildings,
            &mut building_ids,
            &mut npc_ids,
        );

        towns.push(Town {
            id: town_id,
            name: townname,
            number_of_buildings,
            buildings,
        });
    }

    towns
}

// Generate a new graph from a raw graph and a list of towns
fn generate_graph_from_imported_towns(
    graph: &Graph<TownRaw, JourneyInfo>,
    towns: &Vec<Town>,
) -> Graph<Town, JourneyInfo> {
    let mut town_graph = Graph::<Town, JourneyInfo>::new();
    let mut town_map: HashMap<String, NodeIndex> = HashMap::new();

    for town in towns {
        let node_idx = town_graph.add_node(town.clone());
        town_map.insert(town.name.clone(), node_idx);
    }

    for edge in graph.edge_references() {
        let (raw_source, raw_target) = (
            graph.node_weight(edge.source()),
            graph.node_weight(edge.target()),
        );

        if let (Some(raw_source), Some(raw_target)) = (raw_source, raw_target) {
            if let (Some(&source_idx), Some(&target_idx)) = (
                town_map.get(&raw_source.name),
                town_map.get(&raw_target.name),
            ) {
                town_graph.add_edge(source_idx, target_idx, edge.weight().clone());
            }
        }
    }

    town_graph
}

// Function for loading in lists of names from .TXT files
fn load_list(filename: &str) -> Vec<String> {
    let filepath = format!("{}/{}", INPUT_DIR, filename);

    match fs::File::open(filepath) {
        Ok(file) => {
            let reader = io::BufReader::new(file);

            let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
            lines
        }
        Err(e) => {
            eprint!("{}", e);

            let lines = vec!["NO DATA".into()];
            lines
        }
    }
}

// Main function
fn main() {
    let (graph, towns) = generate_towns(NUM_OF_TOWNS);

    match save_graph(&graph, "graph.dot") {
        Ok(result) => println!("{}", result),
        Err(e) => eprintln!("{}", e),
    }
    match save_towns(&towns, "towns.json") {
        Ok(result) => println!("{}", result),
        Err(e) => eprintln!("{}", e),
    }

    //match import("import.dot") {
    //    Ok(imported) => {
    //        match save_graph(&imported.0, "imported_graph.dot") {
    //            Ok(result) => println!("{}", result),
    //            Err(e) => eprintln!("{}", e),
    //        }
    //        match save_towns(&imported.1, "imported_towns.json") {
    //            Ok(result) => println!("{}", result),
    //            Err(e) => eprintln!("{}", e),
    //        }
    //    }
    //    Err(e) => eprint!("{}", e),
    //}
}
