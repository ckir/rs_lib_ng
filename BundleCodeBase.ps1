Remove-Item "rs_lib_ng.txt" -ErrorAction SilentlyContinue
cargo tree > cargotree.txt
dir-to-text --use-gitignore -e "target" -e "Cargo.lock" -e .git .
