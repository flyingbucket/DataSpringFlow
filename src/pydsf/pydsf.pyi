from __future__ import annotations
from typing import Any, List, Optional

class DatasetStatus:
    """Enumeration representing the lifecycle and concurrency state of a dataset.

    Beyond overall health tracking, its busy variants (`Busy*`) serve as a lightweight
    concurrency fence (栅栏机制) for long-running disk operations. Marking a dataset
    as busy before prolonged file access (e.g., model training, data cleaning)
    prevents concurrent workers from interfering or triggering redundant hash verifications.

    Variants:
        Healthy: File contents match Merkle tree and all dependencies are healthy.
        Broken: Current disk file Merkle hash does not match stored records.
        BrokenDeps: Self hash is correct, but one or more dependencies are broken.
        Unverified: Default state when loaded from disk before verification runs.
        NotBusy: Dataset is not used now.
        BusyReading: Dataset is locked by an ongoing read (e.g., epoch training).
        BusyModifying: Dataset is locked by an ongoing file mutation or cleaning task.
        BusyDeleting: Dataset is locked by an ongoing deletion process.
        BusyCreating: Dataset is locked during an initial large-scale registration.

    Example:
        >>> # Erect a read fence before a multi-hour model training task
        >>> service.mark_status("imagenet@v1.0", DatasetStatus.BusyReading)
        >>> try:
        ...     run_long_training_epoch("./data/imagenet")  # Heavy disk read
        >>> finally:
        ...     # Tear down the fence once training completes
        ...     service.mark_status("imagenet@v1.0", DatasetStatus.Healthy)
    """

    Healthy: DatasetStatus
    Broken: DatasetStatus
    BrokenDeps: DatasetStatus
    Unverified: DatasetStatus
    NotBusy: DatasetStatus
    BusyReading: DatasetStatus
    BusyModifying: DatasetStatus
    BusyDeleting: DatasetStatus
    BusyCreating: DatasetStatus

class DataSetVerifyRes:
    """Result of a dataset verification operation.

    Attributes:
        status: The overall health or concurrency status of the target dataset.
        dep_status: A list containing the health status of direct and indirect dependencies.

    Example:
        >>> res = service.verify_deep("nlp_corpus@v2.0")
        >>> if res.status == DatasetStatus.BusyModifying:
        ...     print("Fence active: Upstream dataset is currently being mutated!")
    """

    status: DatasetStatus
    dep_status: list[DatasetStatus]

    def __init__(self, status: DatasetStatus, dep_status: list[DatasetStatus]) -> None:
        """Initializes a verification result object.

        Args:
            status: The primary dataset status.
            dep_status: List of statuses for dependent datasets.

        Example:
            >>> res = DataSetVerifyRes(DatasetStatus.Healthy, [DatasetStatus.Healthy])
            >>> assert res.status == DatasetStatus.Healthy
        """
        ...

class MetaData:
    """Metadata snapshot representing a registered dataset in DataSpringFlow.

    Attributes:
        name: The dataset name (without tag).
        tag: The version tag of the dataset (e.g., "v1.0").
        hash: The cryptographic hex hash representing the dataset contents.
        path: Absolute or relative string path to the actual dataset files.
        description_path: String path to the descriptive markdown or text document.
        script_path: String path to the processing or generation script.
        owner: The username or nickname of the dataset owner (formatted as "user$nick").
        dependencies: List of formatted dataset IDs ("name@tag") that this depends on.
        merkle_tree_path: String path to the serialized Merkle tree file.
        busy_status: Optional active concurrency fence state (e.g., BusyReading).

    Example:
        >>> meta = service.query_meta("imagenet@v1.0")[0].metadata
        >>> if meta.busy_status:
        ...     print(f"Dataset is currently locked: {meta.busy_status}")
    """

    name: str
    tag: str
    hash: str
    path: str
    description_path: str
    script_path: str
    owner: str
    dependencies: list[str]
    merkle_tree_path: str

    def id(self) -> str:
        """Returns the formatted dataset identifier.

        Returns:
            The dataset identifier string in the format "name@tag".

        Example:
            >>> meta = service.query_meta("imagenet@v1.0")[0].metadata
            >>> assert meta.id() == "imagenet@v1.0"
        """
        ...

    def __repr__(self) -> str:
        """Returns a string representation of the metadata.

        Example:
            >>> meta = service.query_meta("imagenet@v1.0")[0].metadata
            >>> print(repr(meta))
        """
        ...

class BackendAddr:
    """Represents the address and connection mode of a DataSpringFlow backend.

    Example:
        >>> local_backend = BackendAddr.local_global()
        >>> remote_backend = BackendAddr.remote_global("https://dsf.lab.org")
    """

    @staticmethod
    def private(username: Optional[str] = None) -> BackendAddr:
        """Creates a Private backend address targeting localized SQLite storage.

        Args:
            username: Optional target username. If None, auto-detects current OS user.

        Returns:
            A configured private `BackendAddr` instance.

        Example:
            >>> private_addr = BackendAddr.private("flyingbucket")
        """
        ...

    @staticmethod
    def local_global() -> BackendAddr:
        """Creates a Local-Global backend address using default system SQLite storage.

        Returns:
            A configured local-global `BackendAddr` instance.

        Example:
            >>> addr = BackendAddr.local_global()
            >>> metas = service.query_meta("data@v1", target_backend=addr)
        """
        ...

    @staticmethod
    def remote_global(server_url: str) -> BackendAddr:
        """Creates a Remote-Global backend address connecting to a remote DSF server.

        Args:
            server_url: The full HTTP/HTTPS URL of the remote DSF server endpoint.

        Returns:
            A configured remote-global `BackendAddr` instance.

        Example:
            >>> remote_addr = BackendAddr.remote_global("https://dsf-server.local:8080")
        """
        ...

class ScopedMetaData:
    """A wrapper containing dataset metadata paired with its corresponding backend source.

    Attributes:
        backend: The backend address where this metadata resides.
        metadata: The dataset metadata object.

    Example:
        >>> scoped = service.query_meta("imagenet@v1.0")[0]
        >>> print(f"Found in {scoped.backend}, fence: {scoped.metadata.busy_status}")
    """

    @property
    def backend(self) -> BackendAddr: ...
    @property
    def metadata(self) -> MetaData: ...
    def __repr__(self) -> str: ...

class ScopedId:
    """A wrapper containing a dataset ID paired with its corresponding backend source.

    Attributes:
        backend: The backend address where this identifier resolves.
        id: The formatted dataset ID string ("name@tag").

    Example:
        >>> refs = service.check_is_referenced("base_data@v1.0")
        >>> print([ref.id for ref in refs])
    """

    @property
    def backend(self) -> BackendAddr: ...
    @property
    def id(self) -> str: ...
    def __repr__(self) -> str: ...

class DSFDataset:
    """Represents an active dataset object within the DataSpringFlow ecosystem.

    Attributes:
        metadata: A snapshot of the dataset's current metadata.
        detailed_status: The latest verification status of the dataset and dependencies.

    Example:
        >>> # Typically managed internally by DSFService during verification
        >>> print(f"Current concurrency fence: {dataset.detailed_status.status}")
    """

    @property
    def metadata(self) -> MetaData: ...
    @property
    def detailed_status(self) -> DataSetVerifyRes: ...
    def verify(self, _backend_auth: Any, _show_diff: bool = False) -> DataSetVerifyRes:
        """Deprecated verification method on the dataset object.

        Args:
            _backend_auth: Authentication binding for the backend.
            _show_diff: Whether to display file-level differences.

        Raises:
            RuntimeError: Always raised. Recommendation is to use `DSFService.verify_deep`.

        Example:
            >>> # Do not call directly; use service.verify_deep() instead.
            >>> pass
        """
        ...

    def __repr__(self) -> str: ...

class DSFService:
    """Main service entrypoint for managing, querying, and verifying datasets.

    Provides core concurrency fencing (`mark_status`), cryptographic hashing
    (`update_merkle`), and topological DAG verification across storage backends.

    Example:
        >>> service = DSFService()
        >>> # Erect fence -> mutate disk -> seal changes -> tear down fence
        >>> service.mark_status("data@v1", DatasetStatus.BusyModifying)
    """

    def __init__(self) -> None:
        """Initializes the DSF service by auto-detecting default backend hierarchy.

        Raises:
            RuntimeError: If the backend architecture fails to initialize or connect.

        Example:
            >>> service = DSFService()
        """
        ...

    def query_meta(
        self, id: str, target_backend: Optional[BackendAddr] = None
    ) -> List[ScopedMetaData]:
        """Queries metadata for a specific dataset identifier across backends.

        Args:
            id: The formatted dataset identifier (e.g., "imagenet@v1.0").
            target_backend: Optional specific backend to query.

        Returns:
            A list of `ScopedMetaData` objects matching the identifier.

        Example:
            >>> metas = service.query_meta("imagenet@v1.0")
            >>> if metas and not metas[0].metadata.busy_status:
            ...     print("Dataset is free and ready for disk operations.")
        """
        ...

    def register(
        self,
        name: str,
        tag: str,
        path: str,
        script_path: str,
        owner_nickname: Optional[str] = None,
        dependencies: Optional[List[str]] = None,
        description_path: Optional[str] = None,
        target_backend: Optional[BackendAddr] = None,
        force_heal: bool = False,
    ) -> None:
        """Registers a new dataset into the DataSpringFlow ecosystem.

        For large directories, this operation initializes disk Merkle hashing.
        It verifies dependency DAG health before committing the initial records.

        Args:
            name: Name of the dataset (without tag or '@').
            tag: Version tag (e.g., "v1.0").
            path: String path to target dataset directory or files.
            script_path: String path to generating script.
            owner_nickname: Optional nickname (formats to `linux_user$nick`).
            dependencies: Optional list of required parent IDs ("name@tag").
            description_path: Optional markdown description path.
            target_backend: Optional backend destination for saving.
            force_heal: If True, forces healing existing broken dependency records.

        Example:
            >>> service.register(
            ...     name="clean_corpus", tag="v1.0",
            ...     path="./data/corpus", script_path="./scripts/clean.py",
            ...     dependencies=["raw_corpus@v1.0"], force_heal=True
            ... )
        """
        ...

    def update_merkle(
        self, id: str, target_backend: Optional[BackendAddr] = None
    ) -> None:
        """Recalculates and seals the Merkle tree hash for a registered dataset.

        Essential step before releasing a `BusyModifying` concurrency fence. It commits
        the new disk state to storage so subsequent verifications return `Healthy`.

        Args:
            id: The dataset identifier ("name@tag").
            target_backend: Optional target backend where the dataset resides.

        Example:
            >>> service.mark_status("corpus@v1", DatasetStatus.BusyModifying)
            >>> run_data_cleaning_pipeline("./data/corpus")  # Alter disk files
            >>> service.update_merkle("corpus@v1")  # Seal new Merkle tree
            >>> service.mark_status("corpus@v1", DatasetStatus.Healthy)
        """
        ...

    def delete_metadata(
        self,
        id: str,
        force: bool = False,
        target_backend: Optional[BackendAddr] = None,
    ) -> None:
        """Deletes a dataset's metadata records from the specified database.

        To ensure safety during long cleanup tasks, consider marking the dataset with
        `BusyDeleting` before removing physical disk files or unregistering metadata.

        Args:
            id: The dataset identifier to delete.
            force: If True, forces deletion even if downstream datasets depend on this.
            target_backend: Optional target backend to execute deletion against.

        Example:
            >>> service.mark_status("old_data@v0.1", DatasetStatus.BusyDeleting)
            >>> import shutil; shutil.rmtree("./data/old_data")  # Remove disk files
            >>> service.delete_metadata("old_data@v0.1", force=True)
        """
        ...

    def verify_deep(
        self,
        id: str,
        show_diff: bool = False,
        target_backend: Optional[BackendAddr] = None,
    ) -> DataSetVerifyRes:
        """Performs deep topological verification of target and its dependency DAG.

        Respects concurrency fences: if any upstream dependency is marked with a non-read
        busy status, verification short-circuits to report the lock without hashing.

        Args:
            id: The dataset identifier to verify.
            show_diff: If True, logs file-level diffs when corruption is detected.
            target_backend: Optional specific backend to run verification against.

        Returns:
            A `DataSetVerifyRes` with the state of target and all dependencies.

        Example:
            >>> res = service.verify_deep("clean_corpus@v1.0")
            >>> if res.status == DatasetStatus.BusyModifying:
            ...     print("Hold off: Upstream dependency is currently being modified!")
        """
        ...

    def verify_self(
        self,
        id: str,
        show_diff: bool = False,
        target_backend: Optional[BackendAddr] = None,
    ) -> DataSetVerifyRes:
        """Performs verification strictly on target dataset, ignoring upstream states.

        Short-circuits immediately if the dataset is under an active concurrency fence
        (such as `BusyModifying`), avoiding collisions with ongoing disk IO.

        Args:
            id: The dataset identifier to verify.
            show_diff: If True, logs file-level diffs when corruption is detected.
            target_backend: Optional specific backend to run verification against.

        Returns:
            A `DataSetVerifyRes` containing only the health of target dataset.

        Example:
            >>> res = service.verify_self("clean_corpus@v1.0")
            >>> if res.status == DatasetStatus.Healthy:
            ...     print("Disk data is verified and safe for concurrent read.")
        """
        ...

    def list_all_metadata(self) -> List[ScopedMetaData]:
        """Lists all dataset metadata registered across available backends.

        Returns:
            A list of all discovered `ScopedMetaData` objects.

        Example:
            >>> active_fences = [
            ...     s.metadata.id() for s in service.list_all_metadata()
            ...     if s.metadata.busy_status
            ... ]
            >>> print(f"Currently locked datasets: {active_fences}")
        """
        ...

    def check_is_referenced(self, target_id: str) -> List[ScopedId]:
        """Finds all downstream datasets that depend on the specified dataset.

        Args:
            target_id: The identifier of the parent dataset to check for references.

        Returns:
            A list of `ScopedId` objects representing dependent datasets.

        Example:
            >>> refs = service.check_is_referenced("raw_corpus@v1.0")
            >>> if refs:
            ...     print("Cannot modify safely: other datasets depend on this!")
        """
        ...

    def mark_status(
        self,
        id: str,
        status: DatasetStatus,
        target_backend: Optional[BackendAddr] = None,
    ) -> None:
        """Manually sets a concurrency fence by marking a dataset's busy status.

        Crucial safeguard when a worker needs prolonged exclusive access to actual
        disk files (e.g., long model training, data generation, or bulk deletion).

        Note:
            Only busy states (`BusyReading`, `BusyModifying`, `BusyDeleting`,
            `BusyCreating`) are permitted when marking status manually.

        Args:
            id: The target dataset identifier ("name@tag").
            status: The target status enum to set. Must be a busy status variant.
            target_backend: Optional target backend where the dataset resides.

        Example:
            >>> # Lock dataset before mutating disk files
            >>> service.mark_status("data@v1", DatasetStatus.BusyModifying)
            >>> # ... execute hours of data processing ...
            >>> service.update_merkle("data@v1")  # Seals changes & restores Healthy
        """
        ...

    def query_dependency_graph(self, root_id: str) -> Any:
        """Queries and verifies the topological dependency graph (DAG) for a dataset.

        Args:
            root_id: The dataset identifier to use as the root of the graph.

        Returns:
            An internal DatasetGraph representation object.

        Example:
            >>> dag = service.query_dependency_graph("pipeline_output@v2.0")
            >>> print("DAG built without cycle or concurrency fence collisions.")
        """
        ...
