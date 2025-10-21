
# MAID Project Agent Documentation

## Überblick
Die **MAID-Agenten** sind dafür verantwortlich, die verschiedenen **Core-Module** und **Plugin-Module** zu implementieren und zu integrieren. Diese Module umfassen **Goose** (für Lasttests), **mistral.rs** (für AI/Inference) und **APIkeys** (für Authentifizierung und Berechtigungen). Der Agent sorgt dafür, dass alle Funktionen wie erwartet ausgeführt werden und unterstützt die Interaktion zwischen den verschiedenen Modulen.

## 1. **Core System: Goose (maid)**

### Funktionalitäten:
- **Lasttest Engine** für APIs und Microservices.
- **Verteiltes Testing (Gaggle-Modus)** zur Durchführung von Lasttests auf mehreren Maschinen.
- **Echtzeit-Überwachung** von Performance-Metriken (CPU, RAM, RPS, Fehlerquoten).
- **Berichterstattung** in verschiedenen Formaten (JSON, CSV, HTML).
- **Flexibilität bei der Konfiguration** via JSON-Schema und **Umgebungsvariablen**.

### Integrationen:
- **mistral.rs**: Integration zur AI-gesteuerten Generierung von Testszenarien und zur Analyse von Ergebnissen.
- **APIkeys**: Authentifizierung und Rollenbasierte Zugriffssteuerung für das Starten und Stoppen von Lasttests.
  
### API-Endpunkte:
- **POST /api/goose/run**: Starten eines Lasttests.
- **POST /api/goose/stop**: Stoppen des Tests.
- **GET /api/goose/status**: Statusabfrage des laufenden Tests.
- **GET /api/goose/history**: Historie der Tests.
  
### Sicherheitsanforderungen:
- Authentifizierung via **JWT** oder **API-Keys**.
- **Rollenbasierte Zugriffskontrolle (RBAC)** für Teststart und -stopp.

---

## 2. **Plugin: mistral.rs (speaker)**

### Funktionalitäten:
- **Multimodale AI-Inferenz**: Verarbeitung von Text, Bild und Audio (Text↔Text, Text+Vision↔Text, Text+Vision+Audio↔Text).
- **Speech-Generierung** und **Bildgenerierung** (Diffusionsmodelle wie FLUX.1).
- **Optimierungen für Performance**: GPU- und CPU-Beschleunigung, Parallelisierung.
- **Quantisierungstechniken** für eine schnellere Ausführung (ISQ, GGUF, GPTQ, etc.).
  
### Integrationen:
- **Goose**: Verwendung von AI-gesteuerten Testszenarien für Lasttests.
- **APIkeys**: Authentifizierung und API-Integration für sichere Nutzung von AI-Modellen.
  
### API-Endpunkte:
- **POST /api/inference/text**: Textbasierte Anfrage an das Modell.
- **POST /api/inference/image**: Anfrage zur Bildgenerierung.
- **POST /api/inference/speech**: Sprachgenerierung auf Anfrage.

### Sicherheitsanforderungen:
- **JWT**-Authentifizierung für Anfragen.
- **API-Schlüssel** für den Zugriff auf Modellressourcen.

---

## 3. **Plugin: APIkeys (Authentication & Authorization)**

### Funktionalitäten:
- **Benutzerverwaltung**: Registrierung, Profilerstellung, Aktivierung, Deaktivierung und Löschung von Benutzern.
- **API-Schlüsselmanagement**: Erstellen, Rotieren, Widerrufen und Verwalten von API-Schlüsseln.
- **Rollen- und Berechtigungsverwaltung**: Erstellen und Zuweisen von benutzerdefinierten Rollen.
- **MFA-Unterstützung**: TOTP- und SMS-basierte Multi-Faktor-Authentifizierung.

### Integrationen:
- **Goose**: Sichert API-Endpunkte zur Verwaltung von Lasttests.
- **mistral.rs**: Absicherung der API zur KI-Inferenz durch Authentifizierung und Rollenmanagement.
  
### API-Endpunkte:
- **POST /api/auth/register**: Benutzerregistrierung.
- **POST /api/auth/login**: Benutzeranmeldung.
- **POST /api/auth/logout**: Benutzerabmeldung.
- **POST /api/apikeys**: Erstellung eines neuen API-Schlüssels.
- **POST /api/roles**: Erstellen einer neuen Rolle.
  
### Sicherheitsanforderungen:
- **JWT**-Authentifizierung für alle API-Endpunkte.
- **Ratenbegrenzung** und **IP-Whitelisting** für API-Schlüssel.
- **Audit-Logging** für alle sicherheitsrelevanten Ereignisse.

---

## 4. **Integrationsanforderungen**

- **Goose** benötigt Zugriff auf die **mistral.rs**-API für AI-gestützte Lasttests.
- **APIkeys** muss die **Benutzerauthentifizierung** für das Starten und Stoppen von Lasttests bereitstellen und **Rollen** für unterschiedliche Zugriffslevels definieren.
- **WebSocket- und REST-API-Kommunikation** zwischen **Goose Core** und den anderen Modulen muss vollständig integriert und dokumentiert sein.

## 5. **BrainDB Agents Overview**

### **1. Core System: HelixDB (BrainDB)**
**Features**:
- **Unified Data Model**: Graph + Vector Database (Nodes, Edges, Embeddings)
- **HelixQL (Query Language)**: Type-safe, compile-time validation, supports Graph Traversals and Vector Search
- **Built-in AI Features**: Embedding Generation, Vector Search (Cosine, Euclidean, Dot Product)
- **Performance**: <1ms query latency, millions of transactions per second (TPS), optimized with LMDB
- **Security**: API Key authentication, compile-time query validation
- **Deployment**: Can be self-hosted or deployed via the provided CLI (`helix check`, `helix push`)

### Agents:
- **Role**: Acts as the foundational **Graph + Vector Database**. Can host and manage documents, embeddings, and support Hybrid Search (Vector + Keyword + Graph).

### Integration:
- **Plugin Bus**: Register plugins (pgML, DB3), expose capabilities, publish events, provide health status.

---

### **2. Machine Learning Plugin: pgML (ParadisML)** 
**Features**:
- **ML Algorithms**: XGBoost, LightGBM, Random Forest, Neural Networks, and more.
- **NLP Tasks**: Text Classification, Zero-shot Classification, Token Classification (NER), Translation, Summarization, Text Generation
- **Vector Database**: Uses PostgreSQL with **pgvector** extension for storing and indexing embeddings.
- **Embedding Generation**: SQL-based API for embedding generation and vector search.
- **Model Training & Inference**: SQL functions `pgml.train()` for model training, `pgml.predict()` for predictions.
- **Performance**: 8-40x faster than HTTP-based ML services.

### Agents:
- **Role**: Offers **in-database machine learning** capabilities, including training, inference, and embedding generation. Integrates seamlessly with **HelixDB** for embedding storage.

### Integration:
- **Hybrid Search**: Uses **pgML** embeddings in **HelixDB** for advanced search capabilities.
- **API Integration**: Available via SQL, integrated with the PostgreSQL ecosystem, and can be accessed via **HelixDB**.

---

### **3. Web3 Archival Plugin: DB3 (Square)** 
**Features**:
- **Web3 Integration**: Full support for **Arweave**, **Polygon**, **zkSync**, and **Scroll** blockchain networks for permanent data storage.
- **Data Nodes**: 
  - **Storage Node**: Handles data rollup to Arweave.
  - **Index Node**: Provides real-time sync and querying of stored data.
- **Security**: Signatures for document updates, WalletConnect integration for secure access.
- **Permanent Storage**: Data is stored permanently on Arweave with **pay-once, store-forever** pricing.
- **SDKs**: Provides a TypeScript SDK for integration.

### Agents:
- **Role**: Provides **permanent storage** for HelixDB documents and metadata. Ensures data permanence and compliance with regulatory standards by using **blockchain-based storage**.

### Integration:
- **PostgresML**: Uses **pgML embeddings** and stores **results** permanently in **DB3**.
- **Data Archival**: All **HelixDB**-related outputs can be archived permanently in **DB3** via blockchain.

---

## 📥 Download the Combined Agent.md
