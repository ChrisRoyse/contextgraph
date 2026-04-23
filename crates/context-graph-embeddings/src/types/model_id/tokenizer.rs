//! Tokenizer families for shared tokenization caching.

/// Tokenizer families for shared tokenization caching.
///
/// Models using the same family can share tokenized inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenizerFamily {
    /// BERT WordPiece tokenization (e5, SPLADE, MiniLM, ColBERT)
    BertWordpiece,
    /// RoBERTa BPE tokenization (KEPLER)
    RobertaBpe,
    /// XLM-RoBERTa SentencePiece tokenization (BGE-M3, multilingual encoders).
    /// vocab_size = 250002, special tokens: <s>=0, <pad>=1, </s>=2, <unk>=3.
    XlmRobertaSentencePiece,
    /// Custom models with no tokenization
    None,
}
