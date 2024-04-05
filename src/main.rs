use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use qubic_miner_benchmark::solution::Solution;

#[derive(Debug, Deserialize)]
struct Config {
    // 定义配置文件中的字段
    wallet:String,
    thread_num:String,
    label:String
}

fn main() {
   // 读取配置文件
   let mut file = match File::open("config.json") {
      Ok(file) => file,
      Err(err) => {
          eprintln!("Error opening file: {}", err);
          return;
      }
  };

  // 读取文件内容
  let mut content = String::new();
  if let Err(err) = file.read_to_string(&mut content) {
      eprintln!("Error reading file: {}", err);
      return;
  }

  // 解析 JSON 配置文件
  let config: Config = match serde_json::from_str(&content) {
      Ok(config) => config,
      Err(err) => {
          eprintln!("Error parsing JSON: {}", err);
          return;
      }
  };

  let mut sol = Solution::new(config.wallet, config.thread_num, config.label);
  sol.update();
  sol.benchmark();

}
