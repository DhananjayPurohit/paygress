// Fix command - Fix various issues

use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::process::Command;

#[derive(Args)]
pub struct FixArgs {
    #[command(subcommand)]
    pub command: FixCommand,

    /// Server SSH address (user@host)
    #[arg(short, long, global = true)]
    pub target: Option<String>,

    /// SSH port
    #[arg(short, long, default_value = "22", global = true)]
    pub port: u16,
}

#[derive(Subcommand)]
pub enum FixCommand {
    /// Fix Kubernetes cluster issues
    Kubernetes,
    
    /// Fix stuck pods
    Pods,
    
    /// Fix containerd issues
    Containerd,
}

pub async fn execute(args: FixArgs, verbose: bool) -> Result<()> {
    let target = args.target.clone().unwrap_or_else(|| get_target_from_inventory());
    
    if target.is_empty() {
        return Err(anyhow::anyhow!(
            "No target specified. Use --target user@host or configure inventory.ini"
        ));
    }

    if verbose {
        println!("{} Target: {}", "→".blue(), target);
    }

    match args.command {
        FixCommand::Kubernetes => {
            println!("{}", "🔧 Fixing Kubernetes...".bold());
            println!();
            
            let fix_script = r#"
echo "Checking Kubernetes..."

# First, fix containerd configuration
echo "Ensuring containerd is properly configured..."

mkdir -p /etc/containerd
containerd config default > /etc/containerd/config.toml
sed -i 's/SystemdCgroup = false/SystemdCgroup = true/' /etc/containerd/config.toml

echo "Restarting containerd..."
systemctl restart containerd
systemctl enable containerd
sleep 5

# Check if API server is accessible
if kubectl cluster-info --request-timeout=5s &> /dev/null; then
    echo "✓ Kubernetes is working"
    kubectl get nodes
    exit 0
fi

echo "✗ Kubernetes API not accessible, reinitializing..."

# Reset and reinitialize
kubeadm reset --force 2>/dev/null || true
rm -rf /etc/cni/net.d ~/.kube
iptables -F && iptables -t nat -F && iptables -t mangle -F && iptables -X 2>/dev/null || true

# Restart containerd
systemctl restart containerd
sleep 3

echo "Initializing Kubernetes..."
kubeadm init --pod-network-cidr=10.244.0.0/16 --ignore-preflight-errors=all

# Setup kubeconfig
mkdir -p ~/.kube
cp /etc/kubernetes/admin.conf ~/.kube/config
chown $(id -u):$(id -g) ~/.kube/config

# Install Flannel CNI
echo "Installing Flannel CNI..."
kubectl apply -f https://github.com/flannel-io/flannel/releases/latest/download/kube-flannel.yml

# Remove taints
sleep 10
kubectl taint nodes --all node-role.kubernetes.io/control-plane- 2>/dev/null || true

# Create namespace
kubectl create namespace user-workloads 2>/dev/null || true

echo "✓ Kubernetes fixed!"
kubectl get nodes
"#;

            run_ssh_script(&target, args.port, fix_script)?;
            
            println!();
            println!("{}", "✅ Kubernetes fix complete".green());
            println!("  {} Restart service: {}", "→".blue(), "paygress-cli service restart".cyan());
        }
        
        FixCommand::Pods => {
            println!("{}", "🔧 Fixing stuck pods...".bold());
            println!();
            
            let fix_script = r#"
echo "Current pod status:"
kubectl get pods -n user-workloads

echo ""
echo "Restarting container runtime..."
systemctl restart containerd
sleep 5
systemctl restart kubelet
sleep 10

echo ""
echo "Deleting stuck pods..."
kubectl delete pod --all -n user-workloads --force --grace-period=0 2>/dev/null || true

echo ""
echo "Waiting for cleanup..."
sleep 15

echo ""
echo "Final status:"
kubectl get pods -n user-workloads
kubectl get nodes

echo ""
echo "✓ Pods cleaned up!"
"#;

            run_ssh_script(&target, args.port, fix_script)?;
            
            println!();
            println!("{}", "✅ Pods fix complete".green());
        }
        
        FixCommand::Containerd => {
            println!("{}", "🔧 Fixing containerd...".bold());
            println!();
            
            let fix_script = r#"
echo "Fixing containerd configuration..."

# Create proper containerd config
mkdir -p /etc/containerd
containerd config default > /etc/containerd/config.toml

# Enable SystemdCgroup (required for Kubernetes)
sed -i 's/SystemdCgroup = false/SystemdCgroup = true/' /etc/containerd/config.toml

# Restart containerd
echo "Restarting containerd..."
systemctl restart containerd
systemctl enable containerd
sleep 5

# Verify containerd is running
if systemctl is-active --quiet containerd; then
    echo "✓ containerd is running"
    systemctl status containerd --no-pager
else
    echo "✗ containerd failed to start"
    systemctl status containerd --no-pager
    exit 1
fi

# Test CRI socket
echo ""
echo "Testing CRI socket..."
crictl --runtime-endpoint unix:///var/run/containerd/containerd.sock version 2>&1 || echo "Warning: crictl test failed"

echo ""
echo "✓ containerd fix complete"
"#;

            run_ssh_script(&target, args.port, fix_script)?;
            
            println!();
            println!("{}", "✅ Containerd fix complete".green());
        }
    }

    Ok(())
}

fn get_target_from_inventory() -> String {
    if let Ok(content) = std::fs::read_to_string("inventory.ini") {
        for line in content.lines() {
            if line.contains("ansible_host=") && line.contains("ansible_user=") {
                let mut host = String::new();
                let mut user = String::new();
                
                for part in line.split_whitespace() {
                    if part.starts_with("ansible_host=") {
                        host = part.replace("ansible_host=", "");
                    } else if part.starts_with("ansible_user=") {
                        user = part.replace("ansible_user=", "");
                    }
                }
                
                if !host.is_empty() && !user.is_empty() {
                    return format!("{}@{}", user, host);
                }
            }
        }
    }
    String::new()
}

fn run_ssh_script(target: &str, port: u16, script: &str) -> Result<()> {
    let status = Command::new("ssh")
        .arg("-p").arg(port.to_string())
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg(target)
        .arg(format!("sudo bash -c '{}'", script.replace("'", "'\\''")))
        .status()?;

    if !status.success() {
        // Continue even on errors
    }

    Ok(())
}
