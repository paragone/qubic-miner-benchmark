use std::fs;
use std::fs::Permissions;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::runtime::Runtime;
use tokio::task;
use reqwest::Client;
use serde_json::{Value, json, to_string_pretty};
use regex::Regex;
use std::thread::{self, Thread};
use std::time::Duration;
use std::process::{Command, Stdio};

const UPDATE_URL:&str = "https://api.github.com/repos/Qubic-Solutions/rqiner-builds/releases/latest";
const UPDATE_LIST_LINUX:[&'static str; 9] =  ["rqiner-x86",
                                        "rqiner-x86-broadwell", 
                                        "rqiner-x86-graniterapids", 
                                        "rqiner-x86-haswell", 
                                        "rqiner-x86-musl", 
                                        "rqiner-x86-sapphirerapids", 
                                        "rqiner-x86-znver2", 
                                        "rqiner-x86-znver3", 
                                        "rqiner-x86-znver4"
                                   ];

const UPDATE_LIST_WIN:[&'static str; 9] =  ["rqiner-x86.exe",
                                            "rqiner-x86-broadwell.exe", 
                                            "rqiner-x86-graniterapids.exe", 
                                            "rqiner-x86-haswell.exe", 
                                            "rqiner-x86-musl.exe", 
                                            "rqiner-x86-sapphirerapids.exe", 
                                            "rqiner-x86-znver2.exe", 
                                            "rqiner-x86-znver3.exe", 
                                            "rqiner-x86-znver4.exe"
                                        ];

fn add_executable_permission(file_path: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(file_path)?;
        let mut permissions = metadata.permissions();

        // 设置用户、组和其他用户的执行权限
        permissions.set_mode(permissions.mode() | 0o111);

        fs::set_permissions(file_path, permissions)?;
    }
    Ok(())
}

// 函数用于执行外部命令并匹配算力
fn execute_and_match(program_name: &str, args: &Vec<&str>, duration: u64) -> Result<Option<f64>, String> {
    // 尝试启动外部程序
    let child = Command::new(format!("./{}", program_name))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // 重定向标准输出以便捕获
        .spawn();

    let mut child = match child {
        Ok(child) => child,
        Err(e) => return Err(format!("Failed to start the process: {}", e)),
    };

    // 等待指定的持续时间
    thread::sleep(Duration::from_secs(duration));

    // 尝试终止进程
    match child.kill() {
        Ok(_) => println!("Process was killed successfully."),
        Err(e) => println!("Failed to kill the process: {}", e),
    }

    // 等待进程退出并获取输出
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => return Err(format!("Failed to wait on the process: {}", e)),
    };
    let mut last_match = None;
    if let Ok(stdout_str) = String::from_utf8(output.stderr) {
        // 使用正则表达式匹配输出
        println!("{}", stdout_str);
        let re = Regex::new(r"Average: (\d+\.\d+) it/s").unwrap();   
        for caps in re.captures_iter(&stdout_str) {
            if let Some(match_) = caps.get(1) {
                if let Ok(average) = match_.as_str().parse::<f64>() {
                    // 更新last_match为当前匹配项，这将在循环结束时保留最后一个匹配项
                    last_match = Some(average);
                }
            }
        }
    }

    Ok(last_match)
}

async fn write_json_to_file(json_data: Vec<serde_json::Value>) -> Result<(),std::io::Error> {
    // 保存 JSON 数据到文件
    let json_str = to_string_pretty(&json_data)?;
    let mut file = File::create("result.json").await?;
    file.write_all(json_str.as_bytes()).await?;
    Ok(())
}

async fn write_to_bat(rqiner:&str, args: &Vec<&str>) -> Result<(), std::io::Error> {
     // 创建一个批处理文件
    let mut file = File::create("run_max.bat").await?;

    // 写入批处理命令
    file.write_all(b"@echo off\n").await?;
    file.write_all(format!("echo Running benchmark for max average rqiner: {}\n", rqiner).as_bytes()).await?;

    let command_line = format!("{} {}", rqiner, args.join(" "));
    file.write_all(command_line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    
    Ok(())
}

async fn write_to_sh(rqiner:&str, args: &Vec<&str>) -> Result<(), std::io::Error> {
    // 创建一个批处理文件
   let mut file = File::create("run_max.sh").await?;

   // 写入批处理命令
   file.write_all(b"#/bin/bash\n").await?;

   let command_line = format!("./{} {}", rqiner, args.join(" "));
   file.write_all(command_line.as_bytes()).await?;
   file.write_all(b"\n").await?;

   add_executable_permission("run_max.sh")?;
   Ok(())
}
pub  struct Solution {
    wallet:String,
    thread_num:String,
    label:String,
    download_urls:Vec<String>
}

impl Solution {
    pub fn new(wallet:String, thread_num:String, label:String) -> Solution {
        Solution {
            wallet,
            thread_num,
            label,
            download_urls:Vec::new()
        }
    }
    pub async fn fetch_and_download_assets(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 创建一个客户端
        let client = Client::new();

        // 构建请求
        let response = client
            .get(UPDATE_URL)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36")
            .send()
            .await?;

        // 检查响应是否成功
        if !response.status().is_success() {
            println!("failed with status: {}", response.status());
            return Ok(());
        }

        // 解析 JSON
        let json_data: Value = response.json().await?;
        let mut update_list = UPDATE_LIST_LINUX;
        if cfg!(target_os = "windows") {
            update_list = UPDATE_LIST_WIN;
        } else if cfg!(target_os = "linux") {
            update_list = UPDATE_LIST_LINUX;// Linux 相关操作
        } else {
            println!("This is neither Windows nor Linux.");
        }

        if let Some(array) = json_data["assets"].as_array() {
            // 使用迭代器遍历数组
            for one in array {
                // 检查数组中的每个值是否是数字
                if let Some(url) = one["browser_download_url"].as_str() {
                    let file_name = url.split('/').last().unwrap_or("");
                    if update_list.contains(&file_name) {
                        println!("URL '{}' 的后缀 '{}' 在 UPDATE_LIST 中", url, file_name);
                        self.download_urls.push(url.to_string())
                    } 
                } else {
                    println!("Not a valid url.");
                }
            }
        } else {
            println!("Not an array.");
        }

        // 创建一个客户端
        let client = reqwest::Client::new();

        // 创建下载任务的句柄
        let mut handles = Vec::new();

        // 遍历下载链接列表并创建下载任务
        for url in self.download_urls.iter() {
            let client = client.clone();
            let url = url.clone();
            let handle = task::spawn(async move {
                
                let mut response = client.get(url.clone()).send().await?;
                let file_name = url.split('/').last().unwrap_or("");
                let mut file = File::create(file_name).await?;
                println!("downloading {} to {}", url.clone(), file_name);
                while let Some(chunk) = response.chunk().await? {
                    file.write_all(&chunk).await?;
                }
                add_executable_permission(file_name);
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
            });
            handles.push(handle);
        }

        // 等待所有下载任务完成
        for handle in handles {
            if let Err(e) = handle.await {
                eprintln!("Error in download task: {}", e);
            }
        }

        Ok(())
    }

    pub fn update(&mut self) {
        // 创建一个同步的 Tokio 运行时
        let runtime = Runtime::new().unwrap();

        // 在 Tokio 运行时中执行异步代码
        runtime.block_on(async {
            let _ = self.fetch_and_download_assets().await;
        });
    }

    pub fn benchmark(&self) {
        let args = vec!["-i", &self.wallet, "-t", &self.thread_num, "-l", &self.label];
        let mut max_average: Option<f64> = None; // 用于存储最大的平均值
        let mut max_rqiner: Option<&str> = None; // 用于存储对应的 rqiner
        let duration = 15;

        let mut json_data = vec![];
        if cfg!(target_os = "windows") {
            
            for rqiner in UPDATE_LIST_WIN {
                println!("start benchmark {}", rqiner);
                match execute_and_match(&rqiner, &args, duration) {
                    Ok(Some(average)) => {println!("Matched average: {} it/s", average);
                    // 保存每个 rqiner 的数据到 JSON 中
                    let data = json!({
                        "rqiner": rqiner,
                        "average": average,
                    });
                    json_data.push(data);

                    // 更新最大的平均值
                    if let Some(max) = max_average {
                        if average > max {
                            max_average = Some(average);
                            max_rqiner = Some(rqiner);
                        }
                    } else {
                        max_average = Some(average);
                        max_rqiner = Some(rqiner);
                    }
                },
                    Ok(None) => println!("No match found"),
                    Err(e) => println!("Error occurred: {}", e),
                }
            }
        } else if cfg!(target_os = "linux") {
            
            for rqiner in UPDATE_LIST_LINUX {
                println!("start benchmark {}", rqiner);
                match execute_and_match(&rqiner, &args, duration) {
                    Ok(Some(average)) => {
                        println!("Matched average: {} it/s", average);
                         // 保存每个 rqiner 的数据到 JSON 中
                        let data = json!({
                            "rqiner": rqiner,
                            "average": average,
                        });
                        json_data.push(data);

                        // 更新最大的平均值
                        if let Some(max) = max_average {
                            if average > max {
                                max_average = Some(average);
                                max_rqiner = Some(rqiner);
                            }
                        } else {
                            max_average = Some(average);
                            max_rqiner = Some(rqiner);
                        }
                    },
                    Ok(None) => println!("No match found"),
                    Err(e) => println!("Error occurred: {}", e),
                }
    
            }
        } else {
            println!("This is neither Windows nor Linux.");
            // 其他平台的处理
        }
        Runtime::new().unwrap().block_on(async {
            let _ = write_json_to_file(json_data).await;
        });
        

        // 输出最大平均值
        if let Some(max) = max_average {
            if let Some(rqiner) = max_rqiner {
                println!("Max average: {} it/s for  {}", max, rqiner);
            }
        }
        if cfg!(target_os = "windows") {
            
            //写bat脚本
            Runtime::new().unwrap().block_on(async {
                let max_rqiner = max_rqiner.expect("max_rqiner is required but was None");
                let _ = write_to_bat(max_rqiner, &args).await;
            });
        } else if cfg!(target_os = "linux") {
            
            //写shell脚本
            Runtime::new().unwrap().block_on(async {
                let max_rqiner = max_rqiner.expect("max_rqiner is required but was None");
                let _ = write_to_sh(max_rqiner, &args).await;
            });
        }
    }
    
}

#[test] 
fn test_update() {
    if cfg!(target_os = "windows") {
        
        // Windows 相关操作
    } else if cfg!(target_os = "linux") {
        
        // Linux 相关操作
    } else {
        println!("This is neither Windows nor Linux.");
        // 其他平台的处理
    }
   let mut solution_test  = Solution::new("PAVAGWGIGXCUKAAKHUCKUSQWJPFABYYZZLMYASJKNAZRFEEQTSBXYEFCTEXG".to_string(), "32".to_string(), "test".to_string());
   solution_test.update();
   solution_test.benchmark();
}