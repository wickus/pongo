extern crate clock_ticks;
extern crate rand;
extern crate sdl2;
extern crate sdl2_gfx;

use rand::distributions::{IndependentSample, Range};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::keyboard::Scancode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Renderer;
use sdl2::Sdl;
use sdl2::TimerSubsystem;
use sdl2::VideoSubsystem;
use std::default::Default;
use std::f32;
use std::path::Path;
use std::thread;

use sdl2_gfx::primitives::DrawRenderer;

struct Ui {
    sdl_ctx: Sdl,
    renderer: Renderer<'static>
}

impl Ui {
    fn new(sdl_ctx: Sdl, renderer: Renderer<'static>) -> Ui {
        Ui { 
            sdl_ctx: sdl_ctx, 
            renderer: renderer 
        }  
    } 

    fn poll_event(&self) -> Option<Event> {
        let mut event_pump = self.sdl_ctx.event_pump().unwrap();
        return event_pump.poll_event();
    }
}

struct Arena {
    color: Color,
    width: f32,
    height: f32
}

impl Arena {
    fn new(color: Color, width: f32, height: f32) -> Arena {
        Arena {
            color: color,
            width: width,
            height: height
        }
    }
}

struct Ball {
    color: Color,
    x: f32,                         // x pixel co-ordinate of top left corner
    y: f32,                         // y pixel co-ordinate of top left corner
    diameter: f32,    
    speed: f32,                     // pixels per second 
    vx: f32,                        // pixels per second
    vy: f32,         
    max_paddle_bounce_angle: f32
}

impl Ball {
    fn new(color: Color, x: f32, y: f32, diameter: f32, speed: f32, vx: f32, 
               vy: f32, max_paddle_bounce_angle: f32) -> Ball {
        Ball { 
            color: color, 
            x: x, 
            y: y, 
            diameter: diameter, 
            speed: speed, 
            vx: vx, 
            vy: vy,
            max_paddle_bounce_angle: max_paddle_bounce_angle
        }
    }
}

struct Paddle {
    color: Color,
    x: f32,         // x pixel co-ordinate of top left corner
    y: f32,         // y pixel co-ordinate of top left corner
    width: f32,     
    height: f32,    
    speed: f32,     // pixels per second
    vy: f32,        // pixels per second
    score: u32
}

impl Paddle {
    fn new(color: Color, x: f32, y: f32, width: f32, height: f32, speed: f32, vy: f32, 
               score: u32) -> Paddle {
        Paddle { 
            color: color,
            x: x, 
            y: y, 
            width: width, 
            height: height, 
            speed: speed, 
            vy: vy, 
            score: score 
        }
    }
}

struct Game {
    ui: Ui,
    fps: u32,
    arena: Arena,
    ball: Ball,
    lpaddle: Paddle,
    rpaddle: Paddle,
    running: bool
}

impl Game {

    /// Create initial game state. 
    fn new(ui: Ui, fps: u32, arena: Arena, ball: Ball, lpaddle: Paddle, 
               rpaddle: Paddle) -> Game { 
        Game { 
            ui: ui, 
            fps: fps, 
            arena: arena,
            ball: ball, 
            lpaddle: lpaddle, 
            rpaddle: rpaddle, 
            running: false 
        }
    }

    /// Start the game and block until finished. 
    fn start(&mut self) {
        self.running = true;
        let mut time_last_invocation = clock_ticks::precise_time_ms();
        while self.running {
            let time_this_invocation = clock_ticks::precise_time_ms();
            let delta_time = time_this_invocation - time_last_invocation;
            self.update(delta_time as f32 / 1000.); 
            self.cap_fps(delta_time);
            time_last_invocation = time_this_invocation;
        } 
    }

    // Called once per frame. 
    fn update(&mut self, dt_sec: f32) {
        self.handle_input(dt_sec);
        self.update_ball_position(dt_sec);
        self.redraw()
    }

    // Handle user input including moving the left paddle. 
    fn handle_input(&mut self, dt_sec: f32) {
        match self.ui.poll_event() {
            Some(event) => {
                match event {
                    Event::Quit{..} => {
                        self.running = false;
                    },
                    Event::MouseMotion{x,y, ..} => {
                        let y = y as f32;
                        let arena = &mut self.arena;
                        let lpaddle = &mut self.lpaddle;
                        lpaddle.y = y; 
                        if lpaddle.y < 0. { 
                            lpaddle.y = 0.; 
                        }
                        else if lpaddle.y + lpaddle.height > arena.height {
                            lpaddle.y = arena.height - lpaddle.height; 
                        }
                    }
                    _ => {}
                }
            },
            None => {}
        }
    }

    // Update the position of the ball and deal with wall and paddle collisions.
    fn update_ball_position(&mut self, dt_sec: f32) {
        let arena = &mut self.arena;
        let ball = &mut self.ball;
        let lpaddle = &mut self.lpaddle;
        let rpaddle = &mut self.rpaddle;
        
        let mut new_ball_x = ball.x + ball.vx * dt_sec;
        let mut new_ball_y = ball.y + ball.vy * dt_sec;

        // Top or bottom wall.
        if new_ball_y < 0. {
            new_ball_y = -new_ball_y;
            ball.vy = -ball.vy;
        } else if new_ball_y + ball.diameter >= arena.height { 
            new_ball_y = arena.height - (new_ball_y + ball.diameter - arena.height) - ball.diameter;
            ball.vy = -ball.vy;
        } 

        // Left or right paddle.
        if new_ball_x < lpaddle.x + lpaddle.width  && ball.x >= lpaddle.x + lpaddle.width {
            let bounce_x = lpaddle.x + lpaddle.width; 
            // The gradient of the straight line from (ball.x,ball.y) to (bounce_x,bounce_y) to
            // (new_ball_x,new_ball_y) stays constant, so we can use that to find the value of the
            // y location of the bounce.
            let bounce_y = (new_ball_y - ball.y) / (new_ball_x - ball.x) * (bounce_x - ball.x) + ball.y;
            if bounce_y >= lpaddle.y && bounce_y <= lpaddle.y + lpaddle.height {
                // Calculate where ball hit relative to center of the paddle.
                let relative_y = lpaddle.y + lpaddle.height / 2. - bounce_y;
                // Use the ratio of the bounce position to half the height of the paddle as an
                // angle multiplier.
                let bounce_angle_multiplier = (relative_y / (lpaddle.height / 2.)).abs();
                let bounce_angle = bounce_angle_multiplier * ball.max_paddle_bounce_angle;
                // Calculate completely new x and y velocities using simple trigonometric
                // identities.
                ball.vx = ball.speed * bounce_angle.cos();
                ball.vy = ball.speed * bounce_angle.sin() * if ball.vy < 0. {-1.} else {1.}; 
                // The imaginary distance travelled beyond the paddle equals the actual distance
                // travelled after the bounce. To calculate the time it took to travel the distance
                // after the bounce, we can take the total time and multiply that by a fraction
                // equal to the ratio of the distance travelled beyond the ball to the total 
                // distance travelled. This would equal the ratio of the hypotenuses of two similar
                // triangles. We don't want to calculate the hypotenuses, but there is a shortcut:
                // We can use the fact that the ratio of corresponding sides for similar triangles
                // are always the same... instead of using the ratio of the hypotenuses, we can use
                // the ratio of the opposite sides. In this case, that'd be the ratio of the y
                // distances travelled:
                let bounce_dt_sec = dt_sec * (new_ball_y - bounce_y) / (new_ball_y - ball.y);
                new_ball_x = bounce_x + ball.vx * bounce_dt_sec;
                new_ball_y = bounce_y + ball.vy * bounce_dt_sec;
            }
        } else if new_ball_x + ball.diameter > rpaddle.x && ball.x + ball.diameter <= rpaddle.x {
            let bounce_x = rpaddle.x; 
            let bounce_y = (new_ball_y - ball.y) / (new_ball_x - ball.x) * (bounce_x - ball.x) + ball.y;
            if bounce_y >= rpaddle.y && bounce_y <= rpaddle.y + rpaddle.height {
                let relative_y = rpaddle.y + rpaddle.height / 2. - bounce_y;
                let bounce_angle_multiplier = (relative_y / (rpaddle.height / 2.)).abs();
                let bounce_angle = bounce_angle_multiplier * ball.max_paddle_bounce_angle;
                ball.vx = ball.speed * bounce_angle.cos() * -1.;
                ball.vy = ball.speed * bounce_angle.sin() * if ball.vy < 0. {-1.} else {1.}; 
                let bounce_dt_sec = dt_sec * (new_ball_y - bounce_y) / (new_ball_y - ball.y);
                new_ball_x = bounce_x + ball.vx * bounce_dt_sec;
                new_ball_y = bounce_y + ball.vy * bounce_dt_sec;
            }
        } 

        // Left or right wall.
        if new_ball_x < 0. { 
            new_ball_x = -new_ball_x;
            ball.vx = -ball.vx;
        } else if new_ball_x + ball.diameter > arena.width { 
            new_ball_x = arena.width - (new_ball_x + ball.diameter - arena.width) - ball.diameter;
            ball.vx = -ball.vx;
        } 

        ball.x = new_ball_x;
        ball.y = new_ball_y;
    }
    
    fn redraw(&mut self) {
        
        // Set the default drawing color (also the color of the background).
        self.ui.renderer.set_draw_color(self.arena.color);

        // Clear the screen.
        self.ui.renderer.clear();

        // Draw the ball.
        let ball = &mut self.ball;
        self.ui.renderer.filled_circle((ball.x + ball.diameter/2.) as i16, 
                                       (ball.y + ball.diameter/2.) as i16, 
                                       (ball.diameter/2.) as i16, 
                                       ball.color);

        // Draw the left paddle.
        let lpaddle = &mut self.lpaddle;
        self.ui.renderer.set_draw_color(lpaddle.color);
        let lpaddle_rect = Rect::new_unwrap(lpaddle.x as i32, 
                                    lpaddle.y as i32, 
                                    lpaddle.width as u32,
                                    lpaddle.height as u32);
        self.ui.renderer.fill_rect(lpaddle_rect);

        // Draw the right paddle.
        let rpaddle = &mut self.rpaddle;
        self.ui.renderer.set_draw_color(rpaddle.color);
        let rpaddle_rect = Rect::new_unwrap(rpaddle.x as i32, 
                                    rpaddle.y as i32, 
                                    rpaddle.width as u32,
                                    rpaddle.height as u32);
        self.ui.renderer.fill_rect(rpaddle_rect);

        // Flip backbuffer to front.
        self.ui.renderer.present();
    }

    // Ensure we run no faster than the desired fps by introducing
    // a delay if necessary.
    fn cap_fps(&self, took_ms: u64) {
        let max_ms = 1000 / self.fps as u64;
        if max_ms > took_ms {
            thread::sleep_ms((max_ms - took_ms) as u32);
        }
    }
}

struct GameBuilder {
    arena_color: Color,
    arena_width: f32,
    arena_height: f32,
    fps: u32,
    ball_color: Color,
    ball_speed: f32,
    ball_diameter: f32,
    lpaddle_color: Color,
    rpaddle_color: Color,
    paddle_offset: f32,
    paddle_width: f32,
    paddle_height: f32,
    paddle_speed: f32,
    max_launch_angle: f32,
    max_bounce_angle: f32
}

impl GameBuilder {

    fn new() -> GameBuilder {
        GameBuilder {
            arena_color: Color::RGB(0xff, 0xff, 0xff),
            arena_width: 480.,
            arena_height: 320.,
            fps: 40,
            ball_color: Color::RGB(0xff, 0xff, 0xff),
            ball_speed: 320.,
            ball_diameter: 10.,
            lpaddle_color: Color::RGB(0xff, 0xff, 0xff),
            rpaddle_color: Color::RGB(0xff, 0xff, 0xff),
            paddle_offset: 4.,
            paddle_width: 10.,
            paddle_height: 80.,
            paddle_speed: 640.,
            max_launch_angle: f32::consts::PI/4.,
            max_bounce_angle: f32::consts::PI/12.
        }
    }

    fn with_arena_dimensions(mut self, width: f32, height: f32) -> GameBuilder {
        self.arena_width = width;
        self.arena_height = height;
        self
    }

    fn with_arena_color(mut self, r: u8, g: u8, b: u8) -> GameBuilder {
        self.arena_color = Color::RGB(r,g,b);
        self
    }

    fn with_fps(mut self, fps: u32) -> GameBuilder {
        self.fps = fps; 
        self
    }
    
    fn with_ball_color(mut self, r: u8, g: u8, b: u8) -> GameBuilder {
        self.ball_color = Color::RGB(r,g,b);
        self
    }

    fn with_ball_speed_per_sec(mut self, speed: f32) -> GameBuilder {
        self.ball_speed = speed; 
        self
    }

    fn with_ball_diameter(mut self, diameter: f32) -> GameBuilder {
        self.ball_diameter = diameter;
        self
    }

    fn with_paddle_offset(mut self, offset: f32) -> GameBuilder {
        self.paddle_offset = offset;
        self
    }

    fn with_paddle_width(mut self, width: f32) -> GameBuilder {
        self.paddle_width = width;
        self
    }
    
    fn with_paddle_height(mut self, height: f32) -> GameBuilder {
        self.paddle_height = height;
        self
    }

    fn with_paddle_speed_per_sec(mut self, speed: f32) -> GameBuilder {
        self.paddle_speed = speed; 
        self
    }

    fn with_left_paddle_color(mut self, r: u8, g: u8, b: u8) -> GameBuilder {
        self.lpaddle_color = Color::RGB(r,g,b);
        self
    }

    fn with_right_paddle_color(mut self, r: u8, g: u8, b: u8) -> GameBuilder {
        self.rpaddle_color = Color::RGB(r,g,b);
        self
    }

    fn with_max_launch_angle_rads(mut self, max_launch_angle: f32) -> GameBuilder {
        self.max_launch_angle = max_launch_angle;
        self
    }
    
    fn with_max_bounce_angle_rads(mut self, max_bounce_angle: f32) -> GameBuilder {
        self.max_bounce_angle = max_bounce_angle;
        self
    }

    fn create_ui(&self) -> Ui {
        let sdl_ctx = sdl2::init().unwrap();
        sdl_ctx.mouse().show_cursor(false);
        //let cursor = sdl2::mouse::Cursor::from_system(sdl2::mouse::SystemCursor::No).unwrap().set(); 
        let video_subsystem = sdl_ctx.video().unwrap();
        let window = video_subsystem.window("pong", 
                self.arena_width as u32, self.arena_height as u32)
                .position_centered()
                .build()
                .unwrap();
        let renderer = window.renderer().build().unwrap();
        Ui::new(sdl_ctx, renderer)
    }

    fn create_arena(&self) -> Arena {
        Arena::new(self.arena_color, self.arena_width, self.arena_height)
    }

    fn create_ball(&self) -> Ball {
        
        // Place ball at center of screen. 
        let diameter = self.ball_diameter;
        let x = self.arena_width/2.;
        let y = self.arena_height/2.;

        let speed = self.ball_speed;
        let mut rng = rand::thread_rng();

        let launch_angle = Range::new(0., self.max_launch_angle).ind_sample(&mut rng);
        let dir = [-1., 1.];

        // Use the sine of the angle to determine the vertical speed. Then, 
        // choose a direction (up or down) to select a vertical velocity.
        let up_or_down = rand::sample(&mut rng, dir.into_iter(),1)[0]; 
        let vy = launch_angle.sin() * speed * up_or_down; 
        let left_or_right = rand::sample(&mut rng, dir.into_iter(),1)[0]; 
        
        // Use Pythagoras to determine the horizontal speed. Then, choose a
        // direction (left or right) to select a horizontal velocity.
        let vx = ((speed * speed) - (vy * vy)).sqrt() * left_or_right;
        Ball::new(self.ball_color, x, y, diameter, speed, vx, vy, self.max_bounce_angle)
    }    

    fn create_left_paddle(&self) -> Paddle {
        let width = self.paddle_width;
        let height = self.paddle_height;
        let x = self.paddle_offset;
        let y = (self.arena_height - height)/2.;
        let speed = self.paddle_speed;
        let vy = 0.;
        let score = 0;
        Paddle::new(self.lpaddle_color, x, y, width, height, speed, vy, score)
    }

    fn create_right_paddle(&self) -> Paddle {
        let width = self.paddle_width;
        let height = self.paddle_height;
        let x = self.arena_width - (self.paddle_offset + width);
        let y = (self.arena_height - height)/2.;
        let speed = self.paddle_speed;
        let vy = 0.;
        let score = 0;
        Paddle::new(self.rpaddle_color, x, y, width, height, speed, vy, score)
    }

    fn build(&self) -> Game {
        Game::new(self.create_ui(), 
                  self.fps, 
                  self.create_arena(),
                  self.create_ball(),
                  self.create_left_paddle(),
                  self.create_right_paddle())
    }
}

fn main() {
    // Moved the todos to Trello. 
    let mut game = GameBuilder::new()
        .with_arena_dimensions(800., 600.)
        .with_arena_color(0x00, 0x00, 0x00)
        .with_fps(40)
        .with_ball_color(0xff, 0xff, 0xff)
        .with_ball_speed_per_sec(400.)
        .with_ball_diameter(11.)
        .with_paddle_offset(4.)
        .with_paddle_width(5.)
        .with_paddle_height(80.)
        .with_paddle_speed_per_sec(1200.)
        .with_left_paddle_color(0xff, 0xff, 0xff)
        .with_right_paddle_color(0xff, 0xff, 0xff)
        .with_max_launch_angle_rads(f32::consts::PI/4.)
        .with_max_bounce_angle_rads(f32::consts::PI/3.)
        .build();
    game.start();
}
