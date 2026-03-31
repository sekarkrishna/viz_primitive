# justembed — Embedding-Native Search

## The Idea

Search that understands meaning, not just keywords. "Find pictures that are red color fruits" returns apples and strawberries — not because they're tagged, but because the embedding model understands what red fruits look like.

## What It Is

A search primitive where every piece of data has an embedding (a vector that captures its meaning). Search is vector similarity, not string matching. The embedding IS the index.

## The Gap

Today's search: text -> tokenize -> inverted index -> keyword match -> rank
Embedding search: data -> embed -> vector index -> similarity search -> rank

The tools exist (FAISS, Annoy, Hnswlib for vector search; CLIP, Sentence-BERT for embeddings) but they're separate pieces you have to glue together. There's no "just search this folder semantically" tool.

## What It Would Look Like

```python
import justembed as je

# Index a folder
index = je.index("/path/to/photos")

# Search by meaning
results = je.search(index, "red color fruits")
results = je.search(index, "sunset over water")
results = je.search(index, "documents about machine learning")

# Cross-modal: search images with text, text with images
results = je.search(index, image_of_apple)  # finds similar images
```

## Technical Requirements

- Embedding models: CLIP (images+text), Sentence-BERT (text), etc.
- Vector index: HNSW or IVF for approximate nearest neighbor
- Storage: embeddings persisted alongside file metadata
- Incremental: new files get embedded automatically

## Relationship to viz_primitive

Minimal. justembed is an ML/search project, not a rendering project. It could use justviz to visualize embedding spaces (UMAP of the index), but the core is embedding + vector search.

## Status

Idea stage. Separate from viz_primitive. Would be its own project/repo.
