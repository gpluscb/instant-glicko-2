# Instant Glicko-2

This crate provides an implementation of the [Glicko-2](https://www.glicko.net/glicko/glicko2.pdf) rating system.
Due to the concept of rating periods, Glicko-2 has the problem that rankings cannot easily be updated instantly after a match concludes.

This implementation aims to solve that problem by allowing fractional rating periods, so that ratings can be updated directly after every game, and not just once a rating period closes.
This draws inspiration from the [rating system implementation](https://github.com/lichess-org/lila/tree/master/modules/rating/src/main/java/glicko2) for open-source chess website [Lichess](https://lichess.org),
as well as two blogpost ([1](https://blog.hypersect.com/the-online-skill-ranking-of-inversus-deluxe/), [2](https://blog.hypersect.com/additional-thoughts-on-skill-ratings/)) by Ryan Juckett on skill ratings for [INVERSUS Deluxe](https://www.inversusgame.com/).

Documentation for the crate can be found on [Docs.rs](https://docs.rs/instant-glicko-2/latest/instant_glicko_2/).

# Examples

Example calculation from [Glickman's paper](https://www.glicko.net/glicko/glicko2.pdf) using `algorithm`:

```rust
use instant_glicko_2::{Parameters, Rating};
use instant_glicko_2::algorithm::{self, PlayerResult};

let parameters = Parameters::default().with_volatility_change(0.5);

// Create our player's rating
let player = Rating::new(1500.0, 200.0, 0.06);

// Create our opponents
// Their volatility is not specified in the paper and it doesn't matter in the calculation,
// so we're just using the default starting volatility.
let opponent_a = Rating::new(1400.0, 30.0, parameters.start_rating().volatility());
let opponent_b = Rating::new(1550.0, 100.0, parameters.start_rating().volatility());
let opponent_c = Rating::new(1700.0, 300.0, parameters.start_rating().volatility());

// Create match results for our player
let results = [
    // Wins first game (score 1.0)
    PlayerResult::new(opponent_a, 1.0),
    // Loses second game (score 0.0)
    PlayerResult::new(opponent_b, 0.0),
    // Loses third game (score 0.0)
    PlayerResult::new(opponent_c, 0.0),
];

// Calculate new rating after 1.0 rating periods
let new_rating = algorithm::rate_player(player, &results, 1.0, parameters);

// The results are close to the results from the paper.
assert!((new_rating.rating() - 1464.06).abs() < 0.01);
assert!((new_rating.deviation() - 151.52).abs() < 0.01);
assert!((new_rating.volatility() - 0.05999).abs() < 0.0001);
```

Different example using `RatingEngine`:

```rust
use std::time::Duration;

use instant_glicko_2::{Parameters, Rating};
use instant_glicko_2::engine::{MatchResult, RatingEngine, RatingResult};

let parameters = Parameters::default();

// Create a RatingEngine with a one day rating period duration
// The first rating period starts instantly
let mut engine = RatingEngine::start_new(
    Duration::from_secs(60 * 60 * 24),
    Parameters::default(),
);

// Register two players
// The first player is relatively strong
let player_1_rating_old = Rating::new(1700.0, 300.0, 0.06);
let player_1 = engine.register_player(player_1_rating_old);
// The second player hasn't played any games
let player_2_rating_old = parameters.start_rating();
let player_2 = engine.register_player(player_2_rating_old);

// They play and player_2 wins
engine.register_result(&RatingResult::new(
    player_1,
    player_2,
    MatchResult::Loss,
));

// Print the new ratings
// Type signatures are needed because we could also work with the internal ScaledRating
// That skips one step of calculation,
// but the rating values are not as pretty and not comparable to the original Glicko ratings
let player_1_rating_new: Rating = engine.player_rating(player_1);
println!("Player 1 old rating: {player_1_rating_old:?}, new rating: {player_1_rating_new:?}");
let player_2_rating_new: Rating = engine.player_rating(player_2);
println!("Player 2 old rating: {player_2_rating_old:?}, new rating: {player_2_rating_new:?}");

// Loser's rating goes down, winner's rating goes up
assert!(player_1_rating_old.rating() > player_1_rating_new.rating());
assert!(player_2_rating_old.rating() < player_2_rating_new.rating());
```
