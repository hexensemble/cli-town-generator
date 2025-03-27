use config::{Config, ConfigError, File};
use inquire::validator::Validation;
use itertools::Itertools;
use petgraph::graph::NodeIndex;
use petgraph::unionfind::UnionFind;
use petgraph::visit::EdgeRef;
use petgraph::Graph;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, Write};
use strum::EnumCount;
use strum_macros::{EnumCount, EnumIter};

// Struct for config settings
#[derive(Debug, Deserialize)]
struct AppConfig {
    seed: String,
    num_of_towns: usize,
    num_of_connections: u32,
    min_distance: u32,
    max_distance: u32,
    cost: u32,
    min_id: u32,
    max_id: u32,
    min_buildings: u32,
    max_buildings: u32,
    min_npcs: u32,
    max_npcs: u32,
    min_rooms: u32,
    max_rooms: u32,
    min_containers: u32,
    max_containers: u32,
    input_dir: String,
    output_dir: String,
}

impl AppConfig {
    fn load(filename: &str) -> Result<Self, ConfigError> {
        print!("Loading settings from file: \"{}\"... ", filename);

        let file_contents = Config::builder()
            .set_default("seed", "Generate")?
            .set_default("num_of_towns", 15)?
            .set_default("num_of_connections", 20)?
            .set_default("min_distance", 10)?
            .set_default("max_distance", 100)?
            .set_default("cost", 5)?
            .set_default("min_id", 1)?
            .set_default("max_id", 100000)?
            .set_default("min_buildings", 5)?
            .set_default("max_buildings", 25)?
            .set_default("min_npcs", 2)?
            .set_default("max_npcs", 10)?
            .set_default("min_rooms", 2)?
            .set_default("max_rooms", 6)?
            .set_default("min_containers", 0)?
            .set_default("max_containers", 4)?
            .set_default("input_dir", "input")?
            .set_default("output_dir", "output")?
            .add_source(File::with_name(filename).required(false))
            .build()?;

        file_contents.try_deserialize::<AppConfig>()
    }
}

// Struct for representing a world. Contains global lists
#[derive(Serialize, Deserialize, Debug)]
struct World {
    towns: HashMap<u32, Town>,
    buildings: HashMap<u32, Building>,
    rooms: HashMap<u32, Room>,
    npcs: HashMap<u32, Npc>,
    containers: HashMap<u32, Container>,
}

// Struct for representing a town
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Town {
    id: u32,
    name: String,
    coords: (u32, u32),
    number_of_buildings: u32,
    buildings: Vec<Building>,
}

// Struct for representing a raw town (containing just a town's name) when importing a DOT file
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
            let distance = parts[0].split_whitespace().next()?.parse().ok()?;
            let cost = parts[1].split_whitespace().next()?.parse().ok()?;
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
    town_id: u32,
    coords: (u32, u32),
    rooms: Vec<Room>,
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
    town_id: u32,
    building_id: u32,
    room_id: Option<u32>,
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

// Struct for representing a room
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Room {
    id: u32,
    town_id: u32,
    building_id: u32,
    npcs: Vec<Npc>,
    containers: Vec<Container>,
}

// Struct for representing a container
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Container {
    id: u32,
    container_type: ContainerType,
    town_id: u32,
    building_id: u32,
    room_id: u32,
}

// Enum for container types
#[derive(Serialize, Deserialize, Debug, Clone, EnumCount, EnumIter)]
enum ContainerType {
    Barrel,
    Crate,
    Chest,
}

// Struct for ID Tracker, keeps track of IDs and generates new ones
struct IdTracker {
    ids: HashSet<u32>,
    rng: StdRng,
}

impl IdTracker {
    fn new(seed: u64) -> Self {
        let rng = StdRng::seed_from_u64(seed);

        Self {
            ids: HashSet::new(),
            rng,
        }
    }

    fn get_new_id(&mut self, settings: &AppConfig) -> u32 {
        let mut id = self.rng.gen_range(settings.min_id..settings.max_id);

        while self.ids.contains(&id) {
            id = self.rng.gen_range(settings.min_id..settings.max_id);
        }
        self.ids.insert(id);

        id
    }
}

// Function for loading in lists of names from .TXT files
fn load_list(settings: &AppConfig, filename: &str) -> Vec<String> {
    let filepath = format!("{}/{}", settings.input_dir, filename);

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

// Function to derive a consistent seed from a word or phrase
fn seed_from_word(word: &str) -> u64 {
    print!("Generating seed from word: \"{}\"... ", word);

    let mut hasher = DefaultHasher::new();
    word.hash(&mut hasher);

    println!("done!");

    hasher.finish()
}

// Function to generate the world
fn generate_world(settings: &AppConfig, seed: u64) -> (Graph<Town, JourneyInfo>, Vec<Town>, World) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut id_tracker = IdTracker::new(seed);

    let (graph, towns) = generate_towns(settings, &mut rng, &mut id_tracker);

    print!("Generating world... ");

    let mut world = World {
        towns: HashMap::new(),
        buildings: HashMap::new(),
        rooms: HashMap::new(),
        npcs: HashMap::new(),
        containers: HashMap::new(),
    };

    world.towns = towns.iter().map(|town| (town.id, town.clone())).collect();

    world.buildings = towns
        .iter()
        .flat_map(|town| {
            town.buildings
                .iter()
                .map(|building| (building.id, building.clone()))
        })
        .collect();

    world.rooms = towns
        .iter()
        .flat_map(|town| {
            town.buildings
                .iter()
                .flat_map(|building| building.rooms.iter().map(|room| (room.id, room.clone())))
        })
        .collect();

    world.npcs = towns
        .iter()
        .flat_map(|town| {
            town.buildings.iter().flat_map(|building| {
                building
                    .rooms
                    .iter()
                    .flat_map(|room| room.npcs.iter().map(|npc| (npc.id, npc.clone())))
            })
        })
        .collect();

    world.containers = towns
        .iter()
        .flat_map(|town| {
            town.buildings.iter().flat_map(|building| {
                building.rooms.iter().flat_map(|room| {
                    room.containers
                        .iter()
                        .map(|container| (container.id, container.clone()))
                })
            })
        })
        .collect();

    println!("done!");

    (graph, towns, world)
}

// Function to generate multiple towns and create a graph
fn generate_towns(
    settings: &AppConfig,
    rng: &mut StdRng,
    id_tracker: &mut IdTracker,
) -> (Graph<Town, JourneyInfo>, Vec<Town>) {
    print!("Generating towns... ");

    let mut towns = Vec::new();

    let prefixes = load_list(settings, "town-prefixes.txt");
    let roots = load_list(settings, "town-roots.txt");
    let suffixes = load_list(settings, "town-suffixes.txt");

    for _ in 0..settings.num_of_towns {
        let town_id = id_tracker.get_new_id(settings);

        let number_of_buildings = rng.gen_range(settings.min_buildings..settings.max_buildings);
        let buildings =
            generate_buildings(settings, rng, id_tracker, &town_id, number_of_buildings);

        towns.push(Town {
            id: town_id,
            name: generate_town_name(rng, &prefixes, &roots, &suffixes),
            coords: (0, 0),
            number_of_buildings,
            buildings,
        });
    }

    println!("done!");

    let (graph, nodes) = generate_graph(settings, rng, towns);

    let list_of_towns = nodes.iter().map(|&node| graph[node].clone()).collect();

    (graph, list_of_towns)
}

// Generate a town name using a prefix-root-suffix combination
fn generate_town_name(
    rng: &mut StdRng,
    prefixes: &[String],
    roots: &[String],
    suffixes: &[String],
) -> String {
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

// Function to generate buildings
fn generate_buildings(
    settings: &AppConfig,
    rng: &mut StdRng,
    id_tracker: &mut IdTracker,
    town_id: &u32,
    number_of_buildings: u32,
) -> Vec<Building> {
    let mut buildings = Vec::new();

    let surnames = load_list(settings, "surnames.txt");
    let shops = load_list(settings, "shops.txt");
    let taverns = load_list(settings, "taverns.txt");
    let temples = load_list(settings, "temples.txt");

    let grid_size = (number_of_buildings as f32).sqrt().ceil() as u32;
    let mut position = (0, 0);

    for _ in 0..number_of_buildings {
        let building_id = id_tracker.get_new_id(settings);

        let building_type = match rng.gen_range(0..BuildingType::COUNT) {
            0 => BuildingType::Residence,
            1 => BuildingType::Shop,
            2 => BuildingType::Tavern,
            3 => BuildingType::Temple,
            _ => BuildingType::Residence,
        };

        let mut building = Building {
            id: building_id,
            name: generate_building_name(
                rng,
                &building_type,
                &surnames,
                &shops,
                &taverns,
                &temples,
            ),
            building_type,
            town_id: *town_id,
            coords: position,
            rooms: Vec::new(),
        };

        let mut npcs = generate_npcs(
            settings,
            rng,
            id_tracker,
            town_id,
            &building_id,
            &building.name,
            &building.building_type,
        );

        building.rooms =
            generate_rooms(settings, rng, id_tracker, town_id, &building_id, &mut npcs);

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
fn generate_building_name(
    rng: &mut StdRng,
    building_type: &BuildingType,
    surnames: &[String],
    shops: &[String],
    taverns: &[String],
    temples: &[String],
) -> String {
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
    settings: &AppConfig,
    rng: &mut StdRng,
    id_tracker: &mut IdTracker,
    town_id: &u32,
    building_id: &u32,
    building_name: &str,
    building_type: &BuildingType,
) -> Vec<Npc> {
    let mut npcs = Vec::new();

    let names_male = load_list(settings, "names-male.txt");
    let names_female = load_list(settings, "names-female.txt");
    let names_unisex = load_list(settings, "names-unisex.txt");
    let surnames = load_list(settings, "surnames.txt");

    let number_of_npcs = match building_type {
        BuildingType::Shop => 1,
        BuildingType::Residence => 2,
        _ => rng.gen_range(settings.min_npcs..settings.max_npcs),
    };

    for _ in 0..number_of_npcs {
        let npc_id = id_tracker.get_new_id(settings);

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
            name: generate_npc_name(
                rng,
                building_name,
                building_type,
                &sex,
                &names_male,
                &names_female,
                &names_unisex,
                &surnames,
            ),
            sex,
            race,
            town_id: *town_id,
            building_id: *building_id,
            room_id: None,
        });
    }

    npcs
}

// Generate an NPC name
#[allow(clippy::too_many_arguments)]
fn generate_npc_name(
    rng: &mut StdRng,
    building_name: &str,
    building_type: &BuildingType,
    npc_sex: &NpcSex,
    names_male: &[String],
    names_female: &[String],
    names_unisex: &[String],
    surnames: &[String],
) -> String {
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

// Generate rooms
fn generate_rooms(
    settings: &AppConfig,
    rng: &mut StdRng,
    id_tracker: &mut IdTracker,
    town_id: &u32,
    building_id: &u32,
    npcs: &mut Vec<Npc>,
) -> Vec<Room> {
    let mut rooms = Vec::new();

    let number_of_rooms = rng.gen_range(settings.min_rooms..settings.max_rooms);

    for _ in 0..number_of_rooms {
        let room_id = id_tracker.get_new_id(settings);

        rooms.push(Room {
            id: room_id,
            town_id: *town_id,
            building_id: *building_id,
            npcs: Vec::new(),
            containers: generate_containers(
                settings,
                rng,
                id_tracker,
                town_id,
                building_id,
                &room_id,
            ),
        });
    }

    npcs.shuffle(rng);

    for mut npc in npcs.drain(..) {
        if let Some(room) = rooms.choose_mut(rng) {
            npc.room_id = Some(room.id);
            room.npcs.push(npc);
        }
    }

    rooms
}

// Generate containers
fn generate_containers(
    settings: &AppConfig,
    rng: &mut StdRng,
    id_tracker: &mut IdTracker,
    town_id: &u32,
    building_id: &u32,
    room_id: &u32,
) -> Vec<Container> {
    let mut containers = Vec::new();

    let num_of_containers = rng.gen_range(settings.min_containers..settings.max_containers);

    for _ in 0..num_of_containers {
        let container_id = id_tracker.get_new_id(settings);

        let container_type = match rng.gen_range(0..ContainerType::COUNT) {
            0 => ContainerType::Barrel,
            1 => ContainerType::Crate,
            2 => ContainerType::Chest,
            _ => ContainerType::Barrel,
        };

        containers.push(Container {
            id: container_id,
            container_type,
            town_id: *town_id,
            building_id: *building_id,
            room_id: *room_id,
        });
    }

    containers
}

// Generate a graph with towns and edges using Kruskal’s Algorithm
fn generate_graph(
    settings: &AppConfig,
    rng: &mut StdRng,
    towns: Vec<Town>,
) -> (Graph<Town, JourneyInfo>, Vec<NodeIndex>) {
    print!("Generating graph... ");

    let mut town_graph = Graph::<Town, JourneyInfo>::new();
    let mut town_nodes = Vec::new();

    for town in towns {
        let node = town_graph.add_node(town);
        town_nodes.push(node);
    }

    // Create a lookup table for constant-time index retrieval
    let node_map: HashMap<NodeIndex, usize> = town_nodes
        .iter()
        .enumerate()
        .map(|(i, &n)| (n, i))
        .collect();

    let mut town_pairs: Vec<(NodeIndex, NodeIndex, u32)> = town_nodes
        .iter()
        .tuple_combinations()
        .map(|(&t1, &t2)| {
            let distance = rng.gen_range(settings.min_distance..settings.max_distance);
            (t1, t2, distance)
        })
        .collect();

    town_pairs.sort_unstable_by_key(|&(_, _, dist)| dist);

    let mut uf = UnionFind::new(town_nodes.len());
    let mut edges_added = 0;

    for (town1, town2, distance) in &town_pairs {
        let idx1 = *node_map.get(town1).expect("Town1 not found in node_map");
        let idx2 = *node_map.get(town2).expect("Town2 not found in node_map");

        if uf.union(idx1, idx2) {
            let cost = settings.cost * distance;
            town_graph.add_edge(
                *town1,
                *town2,
                JourneyInfo {
                    distance: *distance,
                    cost,
                },
            );
            edges_added += 1;
        }
    }

    for (town1, town2, distance) in town_pairs
        .into_iter()
        .skip(edges_added)
        .take(settings.num_of_connections as usize - edges_added)
    {
        let cost = settings.cost * distance;
        town_graph.add_edge(town1, town2, JourneyInfo { distance, cost });
    }

    println!("done!");

    (town_graph, town_nodes)
}

// Save graph to a DOT file
fn save_graph(
    settings: &AppConfig,
    graph: &Graph<Town, JourneyInfo>,
    filename: &str,
) -> Result<String, std::io::Error> {
    print!("Saving graph to file: \"{}\"... ", filename);

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

    let filepath = format!("{}/{}", settings.output_dir, filename);
    fs::create_dir_all(settings.output_dir.clone())?;
    fs::write(filepath, dot_output)?;

    Ok("done!".into())
}

// Save towns to a JSON file
fn save_towns(
    settings: &AppConfig,
    towns: &Vec<Town>,
    filename: &str,
) -> Result<String, std::io::Error> {
    print!("Saving towns to file: \"{}\"... ", filename);

    let json = serde_json::to_string_pretty(towns)?;

    let filepath = format!("{}/{}", settings.output_dir, filename);
    fs::create_dir_all(settings.output_dir.clone())?;
    fs::write(filepath, json)?;

    Ok("done!".into())
}

// Save world to a JSON file
fn save_world(
    settings: &AppConfig,
    world: &World,
    filename: &str,
) -> Result<String, std::io::Error> {
    print!("Saving world to file: \"{}\"... ", filename);

    let json = serde_json::to_string_pretty(world)?;

    let filepath = format!("{}/{}", settings.output_dir, filename);
    fs::create_dir_all(settings.output_dir.clone())?;
    fs::write(filepath, json)?;

    Ok("done!".into())
}

// Import a DOT file and generate Towns and Graph
fn import(settings: &AppConfig, filename: &str, seed: u64) -> ImportResult {
    let imported_raw_graph = load_dot(settings, filename)?;

    let (towns, world) =
        generate_world_from_imported_raw_graph(settings, &imported_raw_graph, seed);
    let graph = generate_graph_from_imported_towns(&imported_raw_graph, &towns);

    Ok((graph, towns, world))
}

// Type alias for the import function's complex return type
type ImportResult = Result<(Graph<Town, JourneyInfo>, Vec<Town>, World), std::io::Error>;

// Load a DOT file
fn load_dot(
    settings: &AppConfig,
    filename: &str,
) -> Result<Graph<TownRaw, JourneyInfo>, std::io::Error> {
    print!("Loading .dot file: \"{}\"... ", filename);
    io::stdout().flush()?;

    let filepath = format!("{}/{}", settings.input_dir, filename);

    let file_content = fs::read_to_string(filepath)?;

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

    println!("done!");

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

// Generate a world from a loaded in DOT file
fn generate_world_from_imported_raw_graph(
    settings: &AppConfig,
    graph: &Graph<TownRaw, JourneyInfo>,
    seed: u64,
) -> (Vec<Town>, World) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut id_tracker = IdTracker::new(seed);

    let towns = generate_towns_from_imported_raw_graph(settings, &mut rng, &mut id_tracker, graph);

    print!("Generating world... ");

    let mut world = World {
        towns: HashMap::new(),
        buildings: HashMap::new(),
        rooms: HashMap::new(),
        npcs: HashMap::new(),
        containers: HashMap::new(),
    };

    world.towns = towns.iter().map(|town| (town.id, town.clone())).collect();

    world.buildings = towns
        .iter()
        .flat_map(|town| {
            town.buildings
                .iter()
                .map(|building| (building.id, building.clone()))
        })
        .collect();

    world.rooms = towns
        .iter()
        .flat_map(|town| {
            town.buildings
                .iter()
                .flat_map(|building| building.rooms.iter().map(|room| (room.id, room.clone())))
        })
        .collect();

    world.npcs = towns
        .iter()
        .flat_map(|town| {
            town.buildings.iter().flat_map(|building| {
                building
                    .rooms
                    .iter()
                    .flat_map(|room| room.npcs.iter().map(|npc| (npc.id, npc.clone())))
            })
        })
        .collect();

    world.containers = towns
        .iter()
        .flat_map(|town| {
            town.buildings.iter().flat_map(|building| {
                building.rooms.iter().flat_map(|room| {
                    room.containers
                        .iter()
                        .map(|container| (container.id, container.clone()))
                })
            })
        })
        .collect();

    println!("done!");

    (towns, world)
}

// Generate towns from a loaded in DOT file
fn generate_towns_from_imported_raw_graph(
    settings: &AppConfig,
    rng: &mut StdRng,
    id_tracker: &mut IdTracker,
    graph: &Graph<TownRaw, JourneyInfo>,
) -> Vec<Town> {
    print!("Generating towns... ");

    let town_names: Vec<String> = graph.node_weights().map(|town| town.name.clone()).collect();

    let mut towns = Vec::new();

    for townname in town_names {
        let town_id = id_tracker.get_new_id(settings);

        let number_of_buildings = rng.gen_range(settings.min_buildings..settings.max_buildings);

        let buildings =
            generate_buildings(settings, rng, id_tracker, &town_id, number_of_buildings);

        towns.push(Town {
            id: town_id,
            name: townname,
            coords: (0, 0),
            number_of_buildings,
            buildings,
        });
    }

    println!("done!");

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

// Menu logic
fn menu(settings: &AppConfig) {
    let message = "Please select an option:".to_string();
    let option1 = "Generate New Towns";
    let option2 = "Import .dot file";
    let option3 = "Exit";
    let options = vec![option1, option2, option3];

    loop {
        println!(" ");

        match inquire::Select::new(&message, options.clone()).prompt() {
            Ok(choice) => {
                if choice == option1 {
                    let seed = seed_from_word(&settings.seed);
                    let (graph, towns, world) = generate_world(settings, seed);

                    match save_graph(settings, &graph, "world.dot") {
                        Ok(result) => println!("{}", result),
                        Err(e) => eprintln!("{}", e),
                    }
                    match save_towns(settings, &towns, "towns.json") {
                        Ok(result) => println!("{}", result),
                        Err(e) => eprintln!("{}", e),
                    }
                    match save_world(settings, &world, "world.json") {
                        Ok(result) => println!("{}", result),
                        Err(e) => eprint!("{}", e),
                    }
                }
                if choice == option2 {
                    let filename_validator = |input: &str| {
                        if input.ends_with(".dot") {
                            Ok(Validation::Valid)
                        } else {
                            Ok(Validation::Invalid("File name must end with .dot".into()))
                        }
                    };

                    match inquire::Text::new("Enter file name to import:")
                        .with_validator(filename_validator)
                        .prompt()
                    {
                        Ok(filename) => {
                            let import_seed = seed_from_word(&settings.seed);

                            match import(settings, &filename, import_seed) {
                                Ok(imported) => {
                                    let (graph, towns, world) = imported;

                                    match save_graph(settings, &graph, "imported_world.dot") {
                                        Ok(result) => println!("{}", result),
                                        Err(e) => eprintln!("{}", e),
                                    }
                                    match save_towns(settings, &towns, "imported_towns.json") {
                                        Ok(result) => println!("{}", result),
                                        Err(e) => eprintln!("{}", e),
                                    }
                                    match save_world(settings, &world, "imported_world.json") {
                                        Ok(result) => println!("{}", result),
                                        Err(e) => eprint!("{}", e),
                                    }
                                }
                                Err(e) => eprintln!("{}", e),
                            }
                        }
                        Err(e) => eprint!("{}", e),
                    }
                }
                if choice == option3 {
                    break;
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                break;
            }
        }
    }
}

// Main function
fn main() {
    let settings = match AppConfig::load("settings.toml") {
        Ok(config) => {
            println!("done!");
            config
        }
        Err(e) => {
            eprintln!("Unable to load settings file: {}", e);
            panic!();
        }
    };

    println!("\nWelcome to CLI Town Generator!\nv1.2\nby HexEnsemble\n\nEdit the settings file to change paramaters.");

    menu(&settings);

    println!("Goodbye!");
}
