# ygo-draw

基于 [typst-ygo](https://github.com/arshtyi/typst-ygo) 的游戏王卡图命令行渲染工具。

## Examples

<table>
    <tbody>
        <tr>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-01.png" alt="1" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-02.png" alt="2" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-03.png" alt="3" style="width:100%;max-width:240px;height:auto;" /></td>
        </tr>
        <tr>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-04.png" alt="4" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-05.png" alt="5" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-06.png" alt="6" style="width:100%;max-width:240px;height:auto;" /></td>
        </tr>
        <tr>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-07.png" alt="7" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-08.png" alt="8" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-09.png" alt="9" style="width:100%;max-width:240px;height:auto;" /></td>
        </tr>
        <tr>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-10.png" alt="1" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-11.png" alt="2" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-12.png" alt="3" style="width:100%;max-width:240px;height:auto;" /></td>
        </tr>
        <tr>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-13.png" alt="1" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-14.png" alt="2" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-15.png" alt="3" style="width:100%;max-width:240px;height:auto;" /></td>
        </tr>
        <tr>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-16.png" alt="1" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-17.png" alt="2" style="width:100%;max-width:240px;height:auto;" /></td>
            <td width="33%"><img src="https://raw.githubusercontent.com/arshtyi/typst-ygo/main/template/card-18.png" alt="3" style="width:100%;max-width:240px;height:auto;" /></td>
        </tr>
    </tbody>
</table>

## Features

- 从文件批量读取 ID、随机选择指定数量，或生成全部 OT/RD 卡图
- 自动下载 typst-ygo、卡片数据和素材资源
- 跳过并打印无效 ID、重复 ID、中心图下载失败和渲染失败
- 输出运行总结，支持一条命令清理资源与渲染结果
- 支持本地构建、Docker 和 GHCR 镜像

## Usage

```txt
Render Yu-Gi-Oh! cards from the command line with typst-ygo

Usage: ygo-draw [OPTIONS]

Options:
      --refresh                      Refresh card data, assets, and typst-ygo before rendering
      --refresh-only                 Refresh resources and exit without rendering cards
      --clean                        Remove all downloaded resources and rendered output, then exit
  -i, --input <INPUT>                Read card IDs from this file, one ID per line [default: cards.txt]
  -r, --random <COUNT>               Render a random selection containing this many cards
      --all <SCOPE>                  Render every available card in the selected scope [possible values: ot, rd, both]
  -o, --output <OUTPUT>              Write rendered card images to this directory [default: output]
      --resource-dir <RESOURCE_DIR>  Store downloaded resources in this directory [default: resources]
  -h, --help                         Print help
  -V, --version                      Print version
```

- 创建 `cards.txt`，每行填写一个十进制卡片 ID。
- 不超过 8 位的 ID 按 OT 处理，超过 8 位的 ID 按 RD 处理。
- 生成的图片默认写入 `output/<id>.png`。
- 使用 `--all ot`、`--all rd` 或 `--all both` 生成对应范围内全部具备中心图的卡片；该选项不能与 `--random` 同时使用。
- 首次使用必须先刷新资源；以后仅在需要更新资源时使用 `--refresh` 或 `--refresh-only`。

## Docker

本地构建：

```bash
docker build -t ygo-draw .
```

也可以使用 GitHub Container Registry 中的默认分支镜像：

```bash
docker pull ghcr.io/arshtyi/ygo-draw-cli:latest
```

将当前目录挂载到容器的 `/data`，并使用当前用户运行，以便生成的文件可由宿主机读写：

```bash
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/data" \
  ghcr.io/arshtyi/ygo-draw-cli:latest --refresh-only

docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/data" \
  ghcr.io/arshtyi/ygo-draw-cli:latest
```
