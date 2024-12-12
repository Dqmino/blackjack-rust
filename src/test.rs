use std::sync::Arc;

#[tokio::main]
async fn main() {
    let game = Arc::new(Mutex::new(BlackjackGame::new()));
    let shared_decision = Arc::new(Mutex::new(None));  // shared decision wrapped in Arc

    let event_dispatcher = Arc::new(EventDispatcher::new());

    // start the event listener in a separate task
    tokio::spawn(listen_for_events(event_dispatcher.clone()));

    // start the game loop in a separate task
    let game_clone = game.clone();
    let shared_decision_clone = shared_decision.clone();  // Clone the Arc to move into the task
    tokio::spawn(async move {
        game_clone.lock().await.play(shared_decision_clone, event_dispatcher.clone()).await;
    });

    tokio::time::sleep(Duration::from_secs(2)).await;
    {
        let mut decision = shared_decision.lock().await;
        *decision = Some(Action::Hit);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    {
        let mut decision = shared_decision.lock().await;
        *decision = Some(Action::Stand);
    }

    // wait for the game loop to finish
    tokio::time::sleep(Duration::from_secs(5)).await;

    let final_game_state = game.lock().await;
    println!("Final game state: {:?}", final_game_state.win_state);
}

async fn listen_for_events(event_dispatcher: Arc<EventDispatcher>) {
    let mut receiver = event_dispatcher.subscribe().await;

    loop {
        match receiver.recv().await {
            Ok(event) => match event {
                GameEvent::PlayerHit(card) => {
                    println!("[][][][] Player hits with card: {:?}", card);
                }
                GameEvent::PlayerStand => {
                    println!("[][][][] Player stands.");
                }
                GameEvent::RoundResult(state) => {
                    println!("[][][][]Round result: {:?}", state);
                }
                GameEvent::PlayerSplit(card) => {
                    println!("[][][][] Player splits with card: {:?}", card);
                }
                GameEvent::PlayerInsure => {
                    println!("[][][][] Player insures.");
                }
                GameEvent::DealerHit(card) => {
                    println!("[][][][] Dealer hits with card: {:?}", card);
                }
                GameEvent::DealerStand => {
                    println!("[][][][] Dealer stands.");
                }
                _ => {}
            },
            Err(_) => break,
        }
    }
}
