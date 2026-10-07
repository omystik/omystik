use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::geo::{clamp_lon, haversine_km};
use crate::paths::{corridors_dir, resolve_graph_path};
use crate::sync_scenario::OsatconScenarioV1;

pub type LatLon = (f64, f64);
pub type Adj = HashMap<i64, Vec<(i64, f64)>>;

pub fn stable_seed_int(seed_str: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed_str.as_bytes());
    let out = hasher.finalize();
    let bytes: [u8; 8] = out[..8].try_into().unwrap();
    u64::from_be_bytes(bytes)
}

#[derive(Debug, Clone)]
pub struct CorridorMeta {
    pub raw: serde_json::Value,
    pub graph_path: String,
}

#[derive(Debug, Clone)]
pub struct TrackPoint {
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone)]
pub struct ConvoyRoute {
    pub mission_seed: String,
    pub force_corridor_name: Option<String>,
    pub speed_kmh: f64,
    pub dt_s: u32,
    pub manifest_path: String,

    pub corridor_meta: Option<CorridorMeta>,
    pub id_to_ll: HashMap<i64, LatLon>,
    pub adj: Adj,
    pub node_path: Vec<i64>,
    pub track: Vec<TrackPoint>,
    pub start_time: DateTime<Utc>,
    pub route_km: f64,
    pub eta_min: f64,

    pub fixed_start_time: Option<DateTime<Utc>>,
}

impl Default for ConvoyRoute {
    fn default() -> Self {
        Self {
            mission_seed: "Convoy".to_string(),
            force_corridor_name: None,
            speed_kmh: 130.0,
            dt_s: 5,
            manifest_path: corridors_dir().join("_manifest.json").display().to_string(),
            corridor_meta: None,
            id_to_ll: HashMap::new(),
            adj: HashMap::new(),
            node_path: Vec::new(),
            track: Vec::new(),
            start_time: Utc::now(),
            route_km: 0.0,
            eta_min: 0.0,
            fixed_start_time: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestEntry {
    id: Option<String>,
    name: Option<String>,
    graph: String,
}

#[derive(Debug, Deserialize)]
struct GraphNode {
    id: i64,
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct GraphEdge {
    u: i64,
    v: i64,
    w_km: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RoadGraph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

impl ConvoyRoute {
    pub fn build(mut self) -> Result<Self> {
        let manifest = self.load_corridor_manifest()?;
        let (graph_path_raw, meta) = self.choose_corridor_graph(&manifest)?;
        let graph_path = resolve_graph_path(&graph_path_raw);

        eprintln!(
            "Selected corridor graph: raw={}, resolved={}",
            graph_path_raw,
            graph_path.display(),
        );
        eprintln!("Selected corridor meta: {:?}", meta);

        self.corridor_meta = Some(CorridorMeta {
            raw: serde_json::to_value(&meta)?,
            graph_path: graph_path.display().to_string(),
        });

        let (id_to_ll, adj) = self.load_road_graph(&graph_path)?;
        self.id_to_ll = id_to_ll;
        self.adj = adj;

        let (start_id, goal_id, _) = self.pick_two_nodes_same_component(40.0, 220.0, 5000)?;
        self.node_path = self.astar_path(start_id, goal_id)?;

        let mut route_km = 0.0;
        for pair in self.node_path.windows(2) {
            let (lat1, lon1) = self.id_to_ll[&pair[0]];
            let (lat2, lon2) = self.id_to_ll[&pair[1]];
            route_km += haversine_km(lat1, lon1, lat2, lon2);
        }
        self.route_km = route_km;
        self.eta_min = (route_km / self.speed_kmh) * 60.0;
        self.track = self.track_from_node_path(&self.node_path)?;
        self.start_time = self.fixed_start_time.unwrap_or_else(Utc::now);

        Ok(self)
    }

    pub fn build_with_scenario(mut self, scenario: &OsatconScenarioV1, session_id: &[u8; 32]) -> Result<Self> {
        self.mission_seed = format!("{}|{}",scenario.scenario_id.clone(), hex::encode(session_id));
        self.fixed_start_time = Some(scenario.start_time());
        self.speed_kmh = scenario.convoy_speed_kmh;
        self.dt_s = scenario.convoy_dt_s;
        self.build()
    }

    pub fn pos_at(&self, t_s: f64) -> Result<LatLon> {
        if self.track.is_empty() {
            bail!("ConvoyRoute not built yet");
        }
        if t_s <= self.track[0].t {
            return Ok((self.track[0].lat, self.track[0].lon));
        }
        if t_s >= self.track[self.track.len() - 1].t {
            let p = &self.track[self.track.len() - 1];
            return Ok((p.lat, p.lon));
        }

        for pair in self.track.windows(2) {
            let p0 = &pair[0];
            let p1 = &pair[1];
            if p0.t <= t_s && t_s <= p1.t {
                let a = (t_s - p0.t) / (p1.t - p0.t).max(1e-9);
                let lat = p0.lat + a * (p1.lat - p0.lat);
                let lon = p0.lon + a * (p1.lon - p0.lon);
                return Ok((lat, lon));
            }
        }

        let p = &self.track[self.track.len() - 1];
        Ok((p.lat, p.lon))
    }

    fn load_corridor_manifest(&self) -> Result<Vec<ManifestEntry>> {
        let s = std::fs::read_to_string(&self.manifest_path)
            .with_context(|| format!("reading manifest {}", self.manifest_path))?;
        let mut v: Vec<ManifestEntry> = serde_json::from_str(&s)?;
        if v.is_empty() {
            bail!("invalid empty corridor manifest");
        }
        v.sort_by(|a, b| {
            let ka = a.id.as_deref().or(a.name.as_deref()).unwrap_or("");
            let kb = b.id.as_deref().or(b.name.as_deref()).unwrap_or("");
            ka.cmp(kb)
        });
        Ok(v)
    }

    fn choose_corridor_graph(&self, manifest: &[ManifestEntry]) -> Result<(String, ManifestEntry)> {
        if let Some(force_name) = &self.force_corridor_name {
            for c in manifest {
                if c.name.as_deref() == Some(force_name.as_str()) {
                    return Ok((c.graph.clone(), ManifestEntry {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        graph: c.graph.clone(),
                    }));
                }
            }
            bail!("force_corridor_name not found: {}", force_name);
        }

        let corridor_seet_time = self.fixed_start_time.unwrap_or_else(Utc::now);
        let minute_seed = corridor_seet_time.format("%Y-%m-%dT%H:%M").to_string();
        let seed = format!("{}|{}", self.mission_seed, minute_seed);
        let idx = (stable_seed_int(&seed) as usize) % manifest.len();
        let c = &manifest[idx];
        Ok((c.graph.clone(), ManifestEntry {
            id: c.id.clone(),
            name: c.name.clone(),
            graph: c.graph.clone(),
        }))
    }

    fn load_road_graph(&self, path: &std::path::Path) -> Result<(HashMap<i64, LatLon>, Adj)> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("reading road graph {}", path.display()))?;

        let g: RoadGraph = serde_json::from_str(&s)
            .with_context(|| format!("parsing road graph {}", path.display()))?;

        eprintln!(
            "RoadGraph loaded: path={}, nodes={}, edges={}",
            path.display(),
            g.nodes.len(),
            g.edges.len(),
        );

        if g.nodes.is_empty() {
            bail!("road graph has zero nodes: {}", path.display());
        }

        let mut id_to_ll = HashMap::new();
        for n in g.nodes {
            id_to_ll.insert(n.id, (n.lat, n.lon));
        }

        let mut adj: Adj = id_to_ll.keys().map(|&nid| (nid, Vec::new())).collect();

        for e in g.edges {
            let (lat1, lon1) = *id_to_ll
                .get(&e.u)
                .ok_or_else(|| anyhow!("missing node {} in {}", e.u, path.display()))?;

            let (lat2, lon2) = *id_to_ll
                .get(&e.v)
                .ok_or_else(|| anyhow!("missing node {} in {}", e.v, path.display()))?;

            let w = e.w_km.unwrap_or_else(|| haversine_km(lat1, lon1, lat2, lon2));

            adj.entry(e.u).or_default().push((e.v, w));
            adj.entry(e.v).or_default().push((e.u, w));
        }

        eprintln!(
            "RoadGraph materialized: id_to_ll={}, adj_keys={}, nonempty_adj={}",
            id_to_ll.len(),
            adj.len(),
            adj.values().filter(|v| !v.is_empty()).count(),
        );

        Ok((id_to_ll, adj))
    }

    fn connected_components(&self) -> Vec<Vec<i64>> {
        let mut seen = HashSet::new();
        let mut comps = Vec::new();

        for &n in self.adj.keys() {
            if seen.contains(&n) {
                continue;
            }
            let mut stack = vec![n];
            let mut comp = Vec::new();
            seen.insert(n);

            while let Some(u) = stack.pop() {
                comp.push(u);
                if let Some(nei) = self.adj.get(&u) {
                    for &(v, _) in nei {
                        if seen.insert(v) {
                            stack.push(v);
                        }
                    }
                }
            }
            comps.push(comp);
        }

        comps.sort_by_key(|c| std::cmp::Reverse(c.len()));
        comps
    }

    fn pick_two_nodes_same_component(
        &self,
        min_km: f64,
        max_km: f64,
        max_tries: usize,
    ) -> Result<(i64, i64, f64)> {
        let comps = self.connected_components();
        if comps.is_empty() {
            bail!("Road graph has no connected components");
        }
        let comp = &comps[0];
        if comp.len() < 2 {
            bail!("Connected component too small");
        }

        let mut rng = SmallRng::seed_from_u64(stable_seed_int(&self.mission_seed));

        for _ in 0..max_tries {
            let a = comp[rng.gen_range(0..comp.len())];
            let b = comp[rng.gen_range(0..comp.len())];
            if a == b {
                continue;
            }
            let (lat1, lon1) = self.id_to_ll[&a];
            let (lat2, lon2) = self.id_to_ll[&b];
            let d = haversine_km(lat1, lon1, lat2, lon2);
            if d >= min_km && d <= max_km {
                return Ok((a, b, d));
            }
        }

        let a = comp[0];
        let (lat1, lon1) = self.id_to_ll[&a];
        let &b = comp
            .iter()
            .max_by(|&&n1, &&n2| {
                let (la1, lo1) = self.id_to_ll[&n1];
                let (la2, lo2) = self.id_to_ll[&n2];
                haversine_km(lat1, lon1, la1, lo1)
                    .partial_cmp(&haversine_km(lat1, lon1, la2, lo2))
                    .unwrap_or(Ordering::Equal)
            })
            .unwrap();

        let (latb, lonb) = self.id_to_ll[&b];
        let d = haversine_km(lat1, lon1, latb, lonb);
        Ok((a, b, d))
    }

    fn astar_path(&self, start_id: i64, goal_id: i64) -> Result<Vec<i64>> {
        #[derive(Copy, Clone, Debug)]
        struct NodeState {
            f: f64,
            g: f64,
            node: i64,
        }

        impl PartialEq for NodeState {
            fn eq(&self, other: &Self) -> bool { self.f == other.f }
        }
        impl Eq for NodeState {}
        impl PartialOrd for NodeState {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                other.f.partial_cmp(&self.f)
            }
        }
        impl Ord for NodeState {
            fn cmp(&self, other: &Self) -> Ordering {
                self.partial_cmp(other).unwrap_or(Ordering::Equal)
            }
        }

        let h = |nid: i64| {
            let (lat, lon) = self.id_to_ll[&nid];
            let (glat, glon) = self.id_to_ll[&goal_id];
            haversine_km(lat, lon, glat, glon)
        };

        let mut open = BinaryHeap::new();
        open.push(NodeState { f: h(start_id), g: 0.0, node: start_id });

        let mut came_from: HashMap<i64, Option<i64>> = HashMap::new();
        let mut gscore: HashMap<i64, f64> = HashMap::new();
        came_from.insert(start_id, None);
        gscore.insert(start_id, 0.0);

        while let Some(NodeState { g, node: cur, .. }) = open.pop() {
            if cur == goal_id {
                let mut path = Vec::new();
                let mut x = Some(cur);
                while let Some(n) = x {
                    path.push(n);
                    x = came_from.get(&n).copied().flatten();
                }
                path.reverse();
                return Ok(path);
            }

            if g > *gscore.get(&cur).unwrap_or(&f64::INFINITY) {
                continue;
            }

            if let Some(neighbors) = self.adj.get(&cur) {
                for &(nxt, w) in neighbors {
                    let ng = g + w;
                    if ng < *gscore.get(&nxt).unwrap_or(&f64::INFINITY) {
                        gscore.insert(nxt, ng);
                        came_from.insert(nxt, Some(cur));
                        open.push(NodeState { f: ng + h(nxt), g: ng, node: nxt });
                    }
                }
            }
        }

        bail!("A* failed: no path found between start and goal")
    }

    fn track_from_node_path(&self, node_path: &[i64]) -> Result<Vec<TrackPoint>> {
        let speed_kms = self.speed_kmh / 3600.0;
        if speed_kms <= 0.0 {
            bail!("speed_kmh must be > 0");
        }

        let mut poly = Vec::with_capacity(node_path.len());
        for nid in node_path {
            poly.push(self.id_to_ll[nid]);
        }

        let mut track = Vec::new();
        let mut t_acc = 0.0;

        for pair in poly.windows(2) {
            let (lat1, lon1) = pair[0];
            let (lat2, lon2) = pair[1];
            let seg_km = haversine_km(lat1, lon1, lat2, lon2);
            let seg_time = seg_km / speed_kms;
            let steps = ((seg_time / self.dt_s as f64).floor() as usize).max(1);

            for k in 0..steps {
                let a = k as f64 / steps as f64;
                let lat = lat1 + a * (lat2 - lat1);
                let lon = latlon_interp_lon(lon1, lon2, a);
                track.push(TrackPoint {
                    t: t_acc + k as f64 * self.dt_s as f64,
                    lat,
                    lon,
                });
            }
            t_acc += steps as f64 * self.dt_s as f64;
        }

        let (lat, lon) = *poly.last().ok_or_else(|| anyhow!("empty polyline"))?;
        track.push(TrackPoint { t: t_acc, lat, lon });
        Ok(track)
    }
}

fn latlon_interp_lon(lon1: f64, lon2: f64, a: f64) -> f64 {
    clamp_lon(lon1 + a * (lon2 - lon1))
}