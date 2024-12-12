use crate::event_dispatcher;
use crate::event_dispatcher::GameEvent;
use event_dispatcher::EventDispatcher;
use rand::prelude::IndexedRandom;
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

#[derive(Debug, Clone)]
pub struct Card {
    pub suit: Suit,
    pub value: u8,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Action {
    Hit,
    Stand,
    Split,
    Insurance,
    Thinking
}
impl Action {
    pub fn from_str(s: &str) -> Option<Action> {
        match s {
            "Hit" => Some(Action::Hit),
            "Stand" => Some(Action::Stand),
            "Split" => Some(Action::Split),
            "Insurance" => Some(Action::Insurance),
            "Thinking" => Some(Action::Thinking),
            _ => None
        }
    }
}

trait Character {
    fn calculate_total_value(&self) -> u8 {
        let mut return_value = self.cards().iter().map(|card| card.value).sum();
        if return_value >= 22 && self.aces() >= 1 {
            return_value -= 10;
        }
        return_value
    }
    fn cards(&self) -> &Vec<Card>;
    fn aces(&self) -> u8 {
        self.cards().iter().filter(|card| card.value == 11).count() as u8
    }
}

#[derive(Debug)]
pub struct Player {
    pub cards: Vec<Card>,
    pub decision: Action,
}

impl Player {
    pub fn can_split(&self) -> bool {
        self.cards.len() == 2 && self.cards[0].value == self.cards[1].value
    }
}

impl Character for Player {
    fn cards(&self) -> &Vec<Card> {
        &self.cards
    }
}

#[derive(Debug)]
pub struct Dealer {
    pub cards: Vec<Card>,
}

impl Character for Dealer {
    fn cards(&self) -> &Vec<Card> {
        &self.cards
    }
}

#[derive(Debug, Clone)]
struct RequeueData {
    pub should_requeue: bool,
    pub card: Option<Card>
}
#[derive(Debug, Clone)]
pub struct WinState {
    pub player: bool,
    pub dealer: bool,
    pub is_decided: bool,
    pub requeue_data: RequeueData
}

#[derive(Debug)]
pub struct BlackjackGame {
    pub player: Player,
    pub dealer: Dealer,
    pub players_turn: bool,
    pub win_state: WinState,
}

impl BlackjackGame {
    pub fn new() -> Self {
        Self {
            player: Player {
                cards: vec![],
                decision: Action::Thinking,
            },
            dealer: Dealer { cards: vec![] },
            players_turn: false,
            win_state: WinState {
                player: false,
                dealer: false,
                is_decided: false,
                requeue_data: RequeueData {
                    should_requeue: false,
                    card: None,
                },
            },
        }
    }

    pub fn viable_for_insurance(&self) -> bool {
        self.dealer.cards.len() == 1 && self.dealer.cards[0].value == 11
    }
    pub async fn play(mut game: Arc<Mutex<BlackjackGame>>, shared_decision: Arc<Mutex<Option<Action>>>, event_dispatcher: Arc<EventDispatcher>) {
        let mut initial_hit = false;
        let mut initial_dealer_hit = false;
        let mut insurance = false;

        while !game.lock().await.win_state.is_decided {
            if !game.lock().await.players_turn {
                if !initial_dealer_hit {
                    game.lock().await.dealer.cards.push(BlackjackGame::random_card());
                    println!(
                        "Dealer [initial/forced] hits! dealer: {}",
                        game.lock().await.dealer.calculate_total_value()
                    );
                    event_dispatcher.dispatch(GameEvent::DealerHit(game.lock().await.dealer.cards.last().unwrap().clone())).await;
                    initial_dealer_hit = true;
                    game.lock().await.players_turn = true;
                    continue;
                }
                if game.lock().await.dealer.calculate_total_value() >= 21 {
                    game.lock().await.check_for_win(event_dispatcher.clone()).await;
                    break;
                }
                if game.lock().await.dealer.calculate_total_value() >= 17 {
                    println!(
                        "Dealer stands! dealer: {}",
                        game.lock().await.dealer.calculate_total_value()
                    );
                    event_dispatcher.dispatch(GameEvent::DealerStand).await;
                    game.lock().await.check_for_win(event_dispatcher.clone()).await;
                    break;

                }

                game.lock().await.dealer.cards.push(BlackjackGame::random_card());
                println!(
                    "Dealer hits! dealer: {}",
                    game.lock().await.dealer.calculate_total_value()
                );
                event_dispatcher.dispatch(GameEvent::DealerHit(game.lock().await.dealer.cards.last().unwrap().clone())).await;

                if insurance {
                    if game.lock().await.dealer.calculate_total_value() == 21 && game.lock().await.dealer.cards.len() == 2 {
                        game.lock().await.win_state.player = true;
                        game.lock().await.win_state.is_decided = true;
                        break;
                    }
                }
                continue;
            }

            if !initial_hit {
                game.lock().await.player.cards.push(BlackjackGame::random_card());
                println!(
                    "Player [initial/forced] hits! player: {}",
                    game.lock().await.player.calculate_total_value()
                );
                let stuff = game.lock().await.player.cards.last().unwrap().clone();
                event_dispatcher.dispatch(GameEvent::PlayerHit(stuff)).await;
                initial_hit = true;
                continue;
            }
            drop(game.lock());
            let player_decision = BlackjackGame::player_decision(shared_decision.clone()).await;
            match player_decision {
                Action::Hit => {
                    game.lock().await.player.cards.push(BlackjackGame::random_card());
                    println!(
                        "Player hits! player: {}",
                        game.lock().await.player.calculate_total_value()
                    );
                    let stuff = game.lock().await.player.cards.last().unwrap().clone();
                    event_dispatcher.dispatch(GameEvent::PlayerHit(stuff)).await;
                    if game.lock().await.player.calculate_total_value() >= 21 {
                        game.lock().await.check_for_win(event_dispatcher.clone()).await;
                        break;
                    }
                }
                Action::Stand => {
                    println!(
                        "Player stands! player: {}",
                        game.lock().await.player.calculate_total_value()
                    );
                    event_dispatcher.dispatch(GameEvent::PlayerStand).await;
                    game.lock().await.players_turn = false;
                }
                Action::Split => {
                    if game.lock().await.player.can_split() {
                        game.lock().await.win_state.requeue_data.should_requeue = true;
                        game.lock().await.win_state.requeue_data.card = Some(game.lock().await.player.cards.pop().unwrap());
                        println!("Player splits! player: {}", game.lock().await.player.calculate_total_value());
                        let stuff = game.lock().await.win_state.requeue_data.card.clone().unwrap();
                        event_dispatcher.dispatch(GameEvent::PlayerSplit(stuff)).await;
                    } else {
                        println!("Player cannot split! player: {}", game.lock().await.player.calculate_total_value());
                    }
                }
                Action::Insurance => {
                    if game.lock().await.viable_for_insurance() {
                        insurance = true;
                        println!("Player insures! player: {}, dealer: {}", game.lock().await.player.calculate_total_value(), game.lock().await.dealer.calculate_total_value());
                        event_dispatcher.dispatch(GameEvent::PlayerInsure).await;
                        game.lock().await.players_turn = false;
                    } else {
                        println!("Player cannot insure! dealer: {}", game.lock().await.dealer.calculate_total_value());
                    }
                }
                _ => {}
            }
        }
    }
    async fn player_decision(
        shared_decision: Arc<Mutex<Option<Action>>>,
    ) -> Action {
        loop {
            if let Some(action) = shared_decision.lock().await.take() {
                return action;
            }
           tokio::time::sleep(Duration::from_millis(1000)).await; // Polling delay
        }
    }
    async fn check_for_win(&mut self, event_dispatcher: Arc<EventDispatcher>) {
        let player_total = self.player.calculate_total_value();
        let dealer_total = self.dealer.calculate_total_value();
        self.decide_winner(player_total, dealer_total, event_dispatcher).await;
    }

    async fn decide_winner(&mut self, player_total: u8, dealer_total: u8, event_dispatcher: Arc<EventDispatcher>) {
        if player_total > 21 {
            println!("Player busts! player: {}", player_total);
            self.win_state.dealer = true;
            self.win_state.is_decided = true;
            event_dispatcher.dispatch(GameEvent::RoundResult(self.win_state.clone())).await;
        } else if dealer_total > 21 {
            println!("Dealer busts! dealer: {}", dealer_total);
            self.win_state.player = true;
            self.win_state.is_decided = true;
            event_dispatcher.dispatch(GameEvent::RoundResult(self.win_state.clone())).await;
        } else if player_total == dealer_total {
            println!("Tie! player: {}, dealer: {}", player_total, dealer_total);
        } else if player_total < dealer_total {
            println!("Dealer wins! dealer: {}", dealer_total);
            self.win_state.dealer = true;
            self.win_state.is_decided = true;
            event_dispatcher.dispatch(GameEvent::RoundResult(self.win_state.clone())).await;
        } else {
            println!("Player wins! player: {}", player_total);
            self.win_state.player = true;
            self.win_state.is_decided = true;
            event_dispatcher.dispatch(GameEvent::RoundResult(self.win_state.clone())).await;
        }
    }


    fn random_card() -> Card {
        let suits = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
        Card {
            suit: suits.choose(&mut rand::rng()).unwrap().clone(),
            value: rand::rng().random_range(2..=11),
        }
    }
}