# Instant Glicko-2

This crate provides an implementation of the [Glicko-2](https://www.glicko.net/glicko/glicko2.pdf) rating system.
Due to the concept of rating periods, Glicko-2 has the problem that rankings cannot easily be updated instantly after a match concludes.

This implementation aims to solve that problem by allowing fractional rating periods, so that ratings can be updated directly after every game, and not just once a rating period closes.
This draws inspiration from the [rating system implementation](https://github.com/lichess-org/lila/tree/master/modules/rating/src/main/glicko2) for open-source chess website [Lichess](https://lichess.org),
as well as two blogpost ([1](https://blog.hypersect.com/the-online-skill-ranking-of-inversus-deluxe/), [2](https://blog.hypersect.com/additional-thoughts-on-skill-ratings/)) by Ryan Juckett on skill ratings for [INVERSUS Deluxe](https://www.inversusgame.com/).

For more on the implementation, I wrote something [here](https://gist.github.com/gpluscb/302d6b71a8d0fe9f4350d45bc828f802).

Documentation for the crate can be found on [Docs.rs](https://docs.rs/instant-glicko-2/latest/instant_glicko_2/).

To use this as a dependency, add the following line to your `Cargo.toml` dependencies:
```toml
instant-glicko-2 = "0.2.0"
```

# Examples

```rust
use instant_glicko_2::{GlickoSettings, PublicRating, IntoWithSettings};
use instant_glicko_2::algorithm::{self, PublicGame};

let settings = GlickoSettings::default().with_volatility_change(0.5);

// Create our player's rating
let mut player = PublicRating::new(1500.0, 200.0, 0.06);

// Create our opponents
// Their volatility is not specified in the paper and it doesn't matter in the calculation,
// so we're just using the default starting volatility.
let opponent_a = PublicRating::new(1400.0, 30.0, settings.start_rating().volatility());
let opponent_b = PublicRating::new(1550.0, 100.0, settings.start_rating().volatility());
let opponent_c = PublicRating::new(1700.0, 300.0, settings.start_rating().volatility());

// Create match results for our player
let results = [
    // Wins first game (score 1.0)
    PublicGame::new(opponent_a, 1.0).into_with_settings(settings),
    // Loses second game (score 0.0)
    PublicGame::new(opponent_b, 0.0).into_with_settings(settings),
    // Loses third game (score 0.0)
    PublicGame::new(opponent_c, 0.0).into_with_settings(settings),
];

// Update rating after rating period
let new_rating: PublicRating = algorithm::rate_games_untimed(player.into_with_settings(settings), &results, 1.0, settings).into_with_settings(settings);

// The rating after the rating period are very close to the results from the paper
assert!((new_rating.rating() - 1464.06).abs() < 0.01);
assert!((new_rating.deviation() - 151.52).abs() < 0.01);
assert!((new_rating.volatility() - 0.05999).abs() < 0.0001);
```
Different example using [`RatingEngine`][engine::RatingEngine]:
```rust
use std::time::Duration;
use instant_glicko_2::{GlickoSettings, PublicRating};
use instant_glicko_2::engine::{MatchResult, RatingEngine};

let settings = GlickoSettings::default();

// Create a RatingEngine with a one day rating period duration
// The first rating period starts instantly
let mut engine = RatingEngine::start_new(GlickoSettings::default());

// Register two players
// The first player is relatively strong
let player_1_rating_old = PublicRating::new(1700.0, 300.0, 0.06);
let player_1 = engine.register_player(player_1_rating_old).0;

// The second player hasn't played any games
let player_2_rating_old = settings.start_rating();
let player_2 = engine.register_player(player_2_rating_old).0;

// They play and player_2 wins
engine.register_result(
    player_1,
    player_2,
    &MatchResult::Loss,
);

// Print the new ratings
// Type signatures are needed because we could also work with the internal InternalRating
// That skips one step of calculation,
// but the rating values are not as pretty and not comparable to the original Glicko ratings
let player_1_rating_new: PublicRating = engine.player_rating(player_1).0;
println!("Player 1 old rating: {player_1_rating_old:?}, new rating: {player_1_rating_new:?}");
let player_2_rating_new: PublicRating = engine.player_rating(player_2).0;
println!("Player 2 old rating: {player_2_rating_old:?}, new rating: {player_2_rating_new:?}");

// Loser's rating goes down, winner's rating goes up
assert!(player_1_rating_old.rating() > player_1_rating_new.rating());
assert!(player_2_rating_old.rating() < player_2_rating_new.rating());
```
