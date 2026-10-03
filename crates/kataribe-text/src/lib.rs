//! 日本語小説テキストの計数・ルビ解析・表記整形・品質指標。
//!
//! このクレートは外部の状態を持たない純粋な関数群だけを提供する。
//! ファイル入出力やネットワークには関与しない。

pub mod count;
pub mod meta;
pub mod normalize;
pub mod quality;
pub mod repetition;
pub mod romaji;
pub mod ruby;
pub mod tokens;
