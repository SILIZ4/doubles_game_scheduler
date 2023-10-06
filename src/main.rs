use itertools::Itertools;
use std::cmp::min;
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::Write;

use rand::prelude::*;

use tqdm::tqdm;

#[derive(Debug)]
struct CumulativeGamesInfo {
    games_played: Vec<Vec<u32>>,
    all_teamates: Vec<Vec<usize>>,
    previous_game: Option<Vec<BTreeSet<usize>>>,
    n_wait: Vec<u32>
}

fn cost_function(games: &Vec<Vec<usize>>, info: &CumulativeGamesInfo) -> f64 {
    let players_played_weight = 2;
    let teamates_weight = 2;
    let last_played_weight = 16;

    let mut cost = 0.;
    for players in games.iter() {
        for (p1, p2) in vec![(0, 1), (2, 3)].into_iter() {
            cost += (teamates_weight*info.all_teamates[players[p1]][players[p2]]) as f64;
        }
        for pair in players.into_iter().permutations(2) {
            cost += (players_played_weight*info.games_played[pair[0].clone()][pair[1].clone()]) as f64;

            cost += match &info.previous_game {
                None => 0.,
                Some(previous_games) => {
                    let mut last_played_together = 0;
                    for previous_game in previous_games.iter() {
                        if previous_game.contains(pair[0]) && previous_game.contains(pair[1]) {
                            last_played_together += 1;
                        }
                    }
                    (last_played_together * last_played_weight) as f64
                }
            };
        }

    }
    cost
}

fn sample_waiting_players(total_players: usize, n_playing: usize, n_wait: &Vec<u32>) -> BTreeSet<usize> {
    let lowest_wait = n_wait.iter().min().unwrap();
    let lowest_waiting_players: BTreeSet<_> = n_wait.iter().enumerate().filter(|(_i, wait)| wait!=&lowest_wait).map(|(i, _wait)| i).collect();

    let mut rng = rand::thread_rng();
    match lowest_waiting_players.len() {
        x if x == n_playing => BTreeSet::from_iter(lowest_waiting_players),
        x if x>n_playing => (0..total_players).choose_multiple(&mut rng, n_playing).into_iter().collect(),
        x if x<n_playing => {
            let all_players = BTreeSet::from_iter(0..total_players);
            let waiting_players: BTreeSet<_> = all_players
                .difference(&lowest_waiting_players)
                .choose_multiple(&mut rng, n_playing-x)
                .into_iter()
                .cloned()
                .collect();
            waiting_players.union(&lowest_waiting_players).cloned().collect()
        },
        _ => BTreeSet::new()
    }
}


// Recursive function adapted by chatGPT
fn enumerate_partitions(
    players: &BTreeSet<usize>,
    num_courts: usize,
    court_size: usize,
    first_player: usize,
    current_partition: &mut Vec<BTreeSet<usize>>,
    all_partitions: &mut Vec<Vec<BTreeSet<usize>>>,
) {
    if num_courts == 0 {
        if players.is_empty() {
            all_partitions.push(current_partition.clone());
        }
        return;
    }

    for court_players in players.iter().cloned().combinations(court_size as usize) {
        let court_set = BTreeSet::from_iter(court_players);
        if current_partition.len() == 1 && !current_partition[0].contains(&first_player) {
            break;
        }

        let remaining_players = BTreeSet::from_iter(players.difference(&court_set).cloned());
        let mut updated_partition = current_partition.clone();
        updated_partition.push(court_set);

        enumerate_partitions(&remaining_players, num_courts - 1, court_size, first_player, &mut updated_partition, all_partitions);
    }
}

fn enumerate_all_games(court_players: &mut Vec<Vec<usize>>, games: &mut Vec<Vec<Vec<usize>>>, court: usize) {
    for permutation in vec![(0, 0), (1, 2), (1, 3)].iter() {
        if court+1 < court_players.len() {
            enumerate_all_games(court_players, games, court+1);
        }
        let current_configuration = court_players[court].clone();
        let (i, j) = permutation.clone();
        court_players[court][i] = current_configuration[j];
        court_players[court][j] = current_configuration[i];
        games.push(court_players.clone());
    }
}

fn find_best_games(total_players: usize, used_courts: usize, court_size: usize, info: &CumulativeGamesInfo) -> Vec<Vec<usize>> {
    let n_playing = used_courts*court_size;
    let mut best_games = (0..used_courts).map(|i| court_size*i).map(|i| vec![i, i+1, i+2, i+3]).collect();
    let mut best_cost = None;

    let mut all_partitions = Vec::new();
    let mut current_partition = Vec::new();

    let mut rng = thread_rng();

    // Exact version:
    //for players in (0..total_players).combinations(n_playing) {
        //let player_set = BTreeSet::from_iter(players);
    let players = sample_waiting_players(total_players, n_playing, &info.n_wait);
    enumerate_partitions(&players, used_courts, court_size, players.first().unwrap().clone(), &mut current_partition, &mut all_partitions);
    for court_players in all_partitions.iter() {
        let mut all_games = Vec::new();
        let mut court_players_vec = Vec::from_iter(court_players.iter().cloned().map(|x| Vec::from_iter(x)));
        enumerate_all_games(&mut court_players_vec, &mut all_games, 0);

        for games in all_games.into_iter() {
            let cost = cost_function(&games, info);
            if best_cost.is_none() || cost < best_cost.unwrap() || cost == best_cost.unwrap() && rng.gen_bool(0.5){
                best_cost = Some(cost);
                best_games = games;
            }
        }
    }
    //}
    best_games
}

fn update_games_info(games: &Vec<Vec<usize>>, game_info: &mut CumulativeGamesInfo, total_players: usize) {
    let mut has_played = vec![false; total_players];

    for players in games.iter() {
        for pair in players.iter().combinations(2) {
            game_info.games_played[pair[0].clone()][pair[1].clone()] += 1;
            game_info.games_played[pair[1].clone()][pair[0].clone()] += 1;
        }
        game_info.all_teamates[players[0].clone()][players[1].clone()] += 1;
        game_info.all_teamates[players[1].clone()][players[0].clone()] += 1;
        game_info.all_teamates[players[2].clone()][players[3].clone()] += 1;
        game_info.all_teamates[players[3].clone()][players[2].clone()] += 1;

        for player in players.into_iter() {
            has_played[player.clone()] = true;
        }
    }
    for (i, played) in has_played.iter().enumerate() {
        if !played {
            game_info.n_wait[i] += 1;
        }
    }
    game_info.previous_game = Some(Vec::from_iter(games.iter().map(|players| BTreeSet::from_iter(players.clone()))));
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => panic!("Missing the total number of players."),
        2 => panic!("Missing the court number."),
        _ => {}
    };

    let total_players = args[1].parse::<usize>().unwrap();
    let court_number = args[2].parse::<usize>().unwrap();
    let games_number = 11;
    let court_size = 4;

    let used_courts = min(total_players/court_size, court_number);

    println!("Finding optimal games for {} players on {} courts.", total_players, court_number);
    // Open file early to detect error before computation
    let output_name = format!("best_games_{}_{}.csv", total_players, court_number);
    let mut file = File::create(output_name)?;


    let mut games_info = CumulativeGamesInfo {
        games_played: vec![vec![0; total_players]; total_players],
        all_teamates: vec![vec![0; total_players]; total_players],
        n_wait: vec![0; total_players],
        previous_game: None
    };

    let default_configuration: Vec<Vec<usize>> = (0..used_courts).map(|i| court_size*i).map(|i| vec![i, i+1, i+2, i+3]).collect();
    update_games_info(&default_configuration, &mut games_info, total_players);

    let mut all_games = vec![default_configuration];
    for _ in tqdm(0..games_number-1) {
        let best_games = find_best_games(total_players, used_courts, court_size, &games_info);
        update_games_info(&best_games, &mut games_info, total_players);
        all_games.push(best_games);
    }


    let header = format!("# Parties optimales pour le mélange de {} joueurs sur {} terrains.\n", total_players, court_number);
    file.write_all(header.as_bytes())?;
    for (i, games) in all_games.into_iter().enumerate() {
        file.write_all(format!("Partie {},", i).as_bytes())?;

        let mut is_playing = vec![false; total_players];
        for players in games.into_iter() {
            let game_str = format!("{}-{} vs {}-{},", players[0], players[1], players[2], players[3]);
            file.write_all(game_str.as_bytes())?;

            for player in players.into_iter() {
                is_playing[player] = true;
            }
        }
        let waiting_str: String = is_playing.into_iter()
            .enumerate()
            .fold(String::new(), |acc, (i, playing)| if !playing {format!("{}{} ", acc, i)} else {acc});
        if !waiting_str.is_empty() {
            file.write_all(format!("En attente: {}", waiting_str).as_bytes())?;
        }
        file.write_all("\n".as_bytes())?;
    }

    Ok(())
}
