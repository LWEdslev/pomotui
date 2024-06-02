use clap::{arg, Parser};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    style::{Color, Modifier, Style},
    Frame, Terminal,
};
#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value_t = 25)]
    work_time: i64,
    #[arg(short, long, default_value_t = 5)]
    short_wait_time: i64,
    #[arg(short, long, default_value_t = 20)]
    long_wait_time: i64,
    #[arg(short, long, default_value_t = 4)]
    cycles: u32,
    #[arg(long, default_value_t = false)]
    dark_mode: bool,
}

fn get_sys_time() -> u128 {
    let now = std::time::SystemTime::now();
    now.duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn convert_millis_to_time(millis: u128) -> String {
    let seconds = millis / 1000;
    let minutes = seconds / 60;
    format!("{:02}:{:02}", minutes, seconds % 60)
}

fn main() -> Result<(), io::Error> {
    // setup terminal
    let args = Parser::parse();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(500);
    let app = App::new(args);
    let res = run_app(&mut terminal, app, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum PomoState {
    Menu,
    Work { time_left: i64 },
    ShortWait { time_left: i64 },
    LongWait { time_left: i64 },
}

impl PomoState {
    fn get_inner(&self) -> Option<i64> {
        match self {
            PomoState::Menu => None,
            PomoState::Work { time_left } => Some(*time_left),
            PomoState::ShortWait { time_left } => Some(*time_left),
            PomoState::LongWait { time_left } => Some(*time_left),
        }
    }
}

struct Settings {
    work_time: i64,
    short_wait_time: i64,
    long_wait_time: i64,
    work_cycles: u32,
    dark_mode: bool,
}

impl Settings {
    fn new(args: Args) -> Self {
        Self {
            work_time: args.work_time * 60 * 1000,
            short_wait_time: args.short_wait_time * 60 * 1000,
            long_wait_time: args.long_wait_time * 60 * 1000,
            work_cycles: args.cycles,
            dark_mode: args.dark_mode,
        }
    }
}

struct App {
    state: PomoState,
    settings: Settings,
    cycle: Option<u32>,
    last_update_time: u128,
    paused: bool,
}

impl App {
    fn new(args: Args) -> Self {
        Self {
            state: PomoState::Menu,
            settings: Settings::new(args),
            cycle: None,
            last_update_time: get_sys_time(),
            paused: false,
        }
    }

    fn start(&mut self) {
        self.state = PomoState::Work {
            time_left: self.settings.work_time,
        };
        self.cycle = Some(self.settings.work_cycles);
        self.last_update_time = get_sys_time();
    }

    fn update(&mut self) {
        let time = get_sys_time();
        let delta = (time - self.last_update_time) as i64;
        self.last_update_time = time;
        if self.paused {
            return;
        }

        let inner_time = match self.state.get_inner() {
            Some(i) => i,
            _ => return,
        };
        let new_inner_time = inner_time - delta;

        // just update the timer
        if new_inner_time.is_positive() {
            self.state = match self.state {
                PomoState::Menu => unreachable!(),
                PomoState::Work { time_left: _ } => PomoState::Work {
                    time_left: new_inner_time,
                },
                PomoState::ShortWait { time_left: _ } => PomoState::ShortWait {
                    time_left: new_inner_time,
                },
                PomoState::LongWait { time_left: _ } => PomoState::LongWait {
                    time_left: new_inner_time,
                },
            };
            return;
        }

        // update the state
        match (self.state.clone(), self.cycle) {
            (PomoState::Work { time_left: _ }, Some(1)) => {
                self.state = PomoState::LongWait {
                    time_left: self.settings.long_wait_time,
                };
            }
            (PomoState::Work { time_left: _ }, Some(_)) => {
                self.state = PomoState::ShortWait {
                    time_left: self.settings.short_wait_time,
                };
            }
            (PomoState::ShortWait { time_left: _ }, Some(i)) => {
                self.state = PomoState::Work {
                    time_left: self.settings.work_time,
                };
                self.cycle = Some(i - 1);
            }
            (PomoState::LongWait { time_left: _ }, _) => {
                self.state = PomoState::Work {
                    time_left: self.settings.work_time,
                };
                self.cycle = Some(self.settings.work_cycles);
            }
            _ => unreachable!(),
        }
    }

    fn get_state_text(&self) -> String {
        if self.paused {
            return "PAUSED".into();
        };
        match self.state {
            PomoState::Menu => "Press 's' to start, 'q' to quit, 'p' to pause".into(),
            PomoState::Work { time_left } => format!(
                "Work: {} - Cycle {}/{}",
                convert_millis_to_time(time_left as u128),
                self.settings.work_cycles - self.cycle.unwrap() + 1,
                self.settings.work_cycles
            ),
            PomoState::ShortWait { time_left } => format!(
                "Short break: {} - Cycle {}/{}",
                convert_millis_to_time(time_left as u128),
                self.settings.work_cycles - self.cycle.unwrap() + 1,
                self.settings.work_cycles
            ),
            PomoState::LongWait { time_left } => {
                format!("Long break: {}", convert_millis_to_time(time_left as u128))
            }
        }
    }

    fn get_ratio(&self) -> f64 {
        if self.paused {
            return 1.;
        };
        match self.state {
            PomoState::Menu => 0.,
            PomoState::Work { time_left } => time_left as f64 / self.settings.work_time as f64,
            PomoState::ShortWait { time_left } => {
                time_left as f64 / self.settings.short_wait_time as f64
            }
            PomoState::LongWait { time_left } => {
                time_left as f64 / self.settings.long_wait_time as f64
            }
        }
    }

    fn get_color(&self) -> tui::style::Color {
        let ratio = self.get_ratio();
        match self.state {
            PomoState::Menu => Color::Gray,
            PomoState::Work { .. } => {
                Color::Rgb((ratio * 255.) as u8, 255 - (ratio * 255.) as u8, 0)
            }
            PomoState::ShortWait { .. } => Color::LightBlue,
            PomoState::LongWait { .. } => Color::LightGreen,
        }
    }
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
                if let KeyCode::Char('s') = key.code {
                    app.start();
                }
                if let KeyCode::Char('p') = key.code {
                    app.paused = !app.paused;
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            app.update();
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let (message, ratio) = (app.get_state_text(), app.get_ratio());
    let size = f.size();
    let color = app.get_color();
    let gauge = tui::widgets::Gauge::default()
        .label(message)
        .gauge_style(
            Style::fg(Style::default(), color)
                .bg(if app.settings.dark_mode {
                    Color::Black
                } else {
                    Color::White
                })
                .add_modifier(Modifier::empty()),
        )
        .ratio(ratio);
    f.render_widget(gauge, size);
}
