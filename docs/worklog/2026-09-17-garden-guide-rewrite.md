# 箱庭のガイドを「遊び方の説明」に（著者の書き直しと、その英語とフォント）

2026-09-17。コミット `ad122f2`。

## 何があったか

著者が `garden/src/guide_text.rs` の日本語を直接書き直した。G6/G6b/G7 で積んだ文面は
「いま画面で起きていること」の説明で、Ruby のタスクが「夜」を待っている、接着コードが 1 行も無い、
最後に体へ書いたタスクが勝つ、といった**仕組みの説明**を含んでいた。著者の版は題からして
「遊び方の説明」で、その 3 点を落とし、文を短く切っている（「〜してしまいます」「別のタスクなので」）。
キーの説明も同じ方針で短くなった（F2「VM の状態を見る」、P「世界の一時停止」、wheel「ズームイン / アウト」）。

判断は著者のもので、こちらの仕事は 2 つ。

1. **英語を日本語に合わせる。** 英語が仕組みを言い、日本語が言わない、という食い違いを残さない。
   落とした 3 点は英語からも落とした。G6b のコメント（3 点を入れた経緯）には、著者の書き直しで
   2 点になった旨を 1 行足した。
2. **フォントを切り直す。** ガイドのフォントはガイドの文字だけを持つサブセット（G6）なので、
   新しい文面の文字が無ければ空白の箱になる。実際に **18 文字**が無かった:
   `うびよグ他内化境変実容態挙状環表装遊`。`tools/subset-font.sh` で切り直して 0 になった
   （64616 → 64944 バイト、グリフ数は 329 のまま。使わなくなった文字と入れ替わった）。

## 確かめ方（残しておく）

fonttools はリポジトリに無いので、使い捨ての venv に入れて cmap を比べた。

```python
from fontTools.ttLib import TTFont
cmap = set(TTFont('crates/rubevy-arena/assets/fonts/NotoSansJP-Guide.subset.ttf').getBestCmap())
chars = {c for p in ['garden/src/guide_text.rs', 'sabibots/src/guide_text.rs',
                     'crates/rubevy-arena/src/guide.rs']
           for c in open(p, encoding='utf-8').read() if ord(c) > 127}
print(''.join(sorted(c for c in chars if ord(c) not in cmap)))   # 空なら足りている
```

`subset-font.sh` の前にこれを走らせると「切り直しが要るか」が分かる。スクリプト自体に
この検査を足すのは、要望が出てからでよい。

## 同時に進んでいたこと

rubevy 側ではマルチ VM（`rubevy/docs/worklog/2026-09-17-multi-vm.md`）の実装担当が、この
リポジトリを `--config` のパッチで自分の枝に向けてビルドし selftest を回していた。
`Cargo.lock` が一時的に書き換わるので、その間 `cargo` をここで走らせると担当の
`git checkout Cargo.lock` と衝突する。ガイドのコンパイル確認は `cargo check --locked` に
同じパッチを付けて行い、ロックファイルには触らなかった。selftest は著者の日本語を含む
バイナリで通っている。
