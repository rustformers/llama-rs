use clap::Parser;
use llama_rs::{InferenceError, InferenceParameters};
use rand::thread_rng;
use rustyline::error::ReadlineError;
use std::{convert::Infallible, io::Write};

#[derive(Debug, Parser)]
pub enum Mode {
    Repl {
        /// Run in REPL mode.
        #[arg(long, short = 'R', default_value_t = false)]
        repl: bool,
    },

    Interactive {
        // Run in interactive mode.
        #[arg(long, short = 'i', default_value_t = false)]
        interactive: bool,
    },
}

impl Mode {
    fn interactive_mode(&self, model: &llama_rs::Model, vocab: &llama_rs::Vocabulary) {
        println!("activated")
        // create a sliding window of context
        // convert initial prompt into tokens
        // convert ai answer into tokens and add into total token count
        // wait for user response
        // repeat
        // issue a warning after the total context is > 2048 tokens
    }

    fn repl_mode(
        &self,
        prompt: &str,
        model: &llama_rs::Model,
        vocab: &llama_rs::Vocabulary,
        params: &InferenceParameters,
    ) {
        // TODO: refactor this to decouple model generation
        // TODO: check run model then store prompt if successful
        let mut rl = rustyline::DefaultEditor::new().unwrap();
        loop {
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    // model generation
                    let mut session = model.start_session(CLI_ARGS.repeat_last_n);
                    // why this?
                    let prompt = prompt.replace("$PROMPT", &line);
                    let mut rng = thread_rng();

                    // TODO: create UI for cli in seperate struct
                    let mut sp = spinners::Spinner::new(spinners::Spinners::Dots2, "".to_string());
                    if let Err(InferenceError::ContextFull) =
                        session.feed_prompt::<Infallible>(model, vocab, params, &prompt, |_| Ok(()))
                    {
                        log::error!("Prompt exceeds context window length.")
                    };
                    sp.stop();

                    let res = session.inference_with_prompt::<Infallible>(
                        model,
                        vocab,
                        params,
                        "",
                        CLI_ARGS.num_predict,
                        &mut rng,
                        |tk| {
                            print!("{tk}");
                            std::io::stdout().flush().unwrap();
                            Ok(())
                        },
                    );
                    println!();

                    if let Err(InferenceError::ContextFull) = res {
                        log::error!("Reply exceeds context window length");
                    }
                }
                Err(ReadlineError::Eof) | Err(ReadlineError::Interrupted) => {
                    break;
                }
                Err(err) => {
                    log::error!("{err}");
                }
            }
        }
    }

    fn run(self) {
        match self {
            Self::Repl { repl } => self.repl_mode(),
            Self::Interactive { interactive } => self.interactive_mode(),
        }
    }
}
