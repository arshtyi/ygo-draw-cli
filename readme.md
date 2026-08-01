# ygo-draw

> 本项目停止维护，仅维护[web](https://github.com/arshtyi/typst-ygo-web)

基于 [typst-ygo](https://github.com/arshtyi/typst-ygo) 的游戏王卡图命令行渲染工具。

## 功能

- 从文件、随机结果或完整卡片库批量生成 OT/RD 卡图
- 自动管理 typst-ygo、卡片数据、素材和中心图
- 跳过无效或失败项目，并输出进度和运行总结
- 支持本地运行、Docker 和 GHCR 镜像

## 使用

```console
Render Yu-Gi-Oh! cards from the command line with typst-ygo

Usage: ygo-draw [OPTIONS]

Options:
      --refresh                      Refresh card data, assets, and typst-ygo before rendering
      --refresh-only                 Refresh resources and exit without rendering cards
      --clean                        Remove all downloaded resources and rendered output, then exit
  -i, --input <INPUT>                Read card IDs from this file, one ID per line [default: cards.txt]
  -r, --random <COUNT>               Render a random selection containing this many cards
      --random-scope <SCOPE>         Limit random selection to the selected card scope [possible values: ot, rd, both]
      --all <SCOPE>                  Render every available card in the selected scope [possible values: ot, rd, both]
  -o, --output <OUTPUT>              Write rendered card images to this directory [default: output]
      --resource-dir <RESOURCE_DIR>  Store downloaded resources in this directory [default: resources]
  -h, --help                         Print help
  -V, --version                      Print version
```

## Docker

本地构建或拉取 GHCR 镜像：

```bash
docker build -t ygo-draw .
docker pull ghcr.io/arshtyi/ygo-draw-cli:latest
```

将当前目录挂载到容器的 `/data`，并使用当前用户运行，以便生成的文件可由宿主机读写：

```bash
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/data" ghcr.io/arshtyi/ygo-draw-cli:latest --refresh-only

docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/data" ghcr.io/arshtyi/ygo-draw-cli:latest
```
