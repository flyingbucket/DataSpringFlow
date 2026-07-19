from __future__ import annotations
from enum import Enum
from typing import Any, Optional

class DatasetStatus:
    """Represents the overall health or verification status of a dataset.

    Attributes:
        Healthy: Dataset is intact, verified, and free of corruption.
        Broken: Dataset files have failed verification or are missing.
        BrokenDeps: Target dataset is fine, but one or more upstream dependencies are broken.
        Unverified: Dataset has been registered or modified but not yet verified.
    """

    Healthy: DatasetStatus
    Broken: DatasetStatus
    BrokenDeps: DatasetStatus
    Unverified: DatasetStatus

    def __eq__(self, other: Any) -> bool: ...

class BusyStatus(int, Enum):
    """Represents the active concurrency fence state of a dataset.

    Used to lock datasets during disk operations to prevent race conditions
    and concurrent modification conflicts across storage backends.

    Attributes:
        Free: Dataset is idle and available for reading or writing.
        Reading: Dataset is actively being read by one or more processes.
        Modifying: Dataset files are currently being modified on disk.
        Deleting: Dataset files or metadata are in the process of being deleted.
        Creating: Dataset is currently being generated or initially written.

    Example:
        ```python
         service = DSFService()
         service.mark_status("data@v1", BusyStatus.Modifying)

        ```
    """

    Free = 0
    Reading = 1
    Modifying = 2
    Deleting = 3
    Creating = 4

class DataSetVerifyRes:
    """Result of a dataset verification operation.

    Attributes:
        status: The primary health or concurrency status of the target dataset.
        dep_status: A list containing the health statuses of direct and indirect dependencies.

    Example:
        ```python
         res = service.verify_deep("nlp_corpus@v2.0")
         if res.status == DatasetStatus.Broken:
             print("Dataset corruption detected!")

        ```
    """

    status: DatasetStatus
    dep_status: list[DatasetStatus]

    def __init__(self, status: DatasetStatus, dep_status: list[DatasetStatus]) -> None:
        """Initializes a verification result object.

        Args:
            status: The primary dataset status.
            dep_status: List of statuses for dependent datasets.

        Example:
            ```python
             res = DataSetVerifyRes(DatasetStatus.Healthy, [DatasetStatus.Healthy])
             assert res.status == DatasetStatus.Healthy

            ```
        """
        ...

    def __repr__(self) -> str: ...

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

    Example:
        ```python

         meta = service.query_meta("imagenet@v1.0")[0].metadata
         print(f"Dataset path: {meta.path}")

        ```
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
            ```python
             meta = service.query_meta("imagenet@v1.0")[0].metadata
             assert meta.id() == "imagenet@v1.0"

            ```
        """
        ...

    def __repr__(self) -> str: ...

class BackendAddr:
    """Represents the address and connection mode of a DataSpringFlow backend.

    Example:
        ```python
         local_backend = BackendAddr.local_global()
         remote_backend = BackendAddr.remote_global("https://dsf.lab.org")

        ```
    """

    @staticmethod
    def private(username: Optional[str] = None) -> BackendAddr:
        """Creates a Private backend address targeting localized SQLite storage.

        Args:
            username: Optional target username. If None, auto-detects current OS user.

        Returns:
            A configured private `BackendAddr` instance.

        Example:
            ```python
             private_addr = BackendAddr.private("flyingbucket")

            ```
        """
        ...

    @staticmethod
    def local_global() -> BackendAddr:
        """Creates a Local-Global backend address using default system SQLite storage.

        Returns:
            A configured local-global `BackendAddr` instance.

        Example:
            ```python
             addr = BackendAddr.local_global()
             metas = service.query_meta("data@v1", target_backend=addr)

            ```
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
            ```python
             remote_addr = BackendAddr.remote_global("https://dsf-server.local:8080")

            ```
        """
        ...

    def __repr__(self) -> str: ...

class ScopedMetaData:
    """A wrapper containing dataset metadata paired with its corresponding backend source.

    Attributes:
        backend: The backend address where this metadata resides.
        metadata: The dataset metadata object.

    Example:
        ```python
         scoped = service.query_meta("imagenet@v1.0")[0]
         print(f"Found in {scoped.backend}")

        ```
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
        ```python
         refs = service.check_is_referenced("base_data@v1.0")
         print([ref.id for ref in refs])

        ```
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
        ```python
         # Typically managed internally by DSFService during verification
         print(f"Current verification status: {dataset.detailed_status.status}")

        ```
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
        """
        ...

    def __repr__(self) -> str: ...

class DSFService:
    """Main service entrypoint for managing, querying, and verifying datasets.

    Provides core concurrency fencing (`mark_status`), cryptographic hashing
    (`update_merkle`), and topological DAG verification across storage backends.

    Example:
        ```python
         service = DSFService()
         # Erect fence -> mutate disk -> seal changes -> tear down fence
         service.mark_status("data@v1", BusyStatus.Modifying)
         # ... modify disk files ...
         service.update_merkle("data@v1")
         service.mark_status("data@v1", BusyStatus.Free)

        ```
    """

    def __init__(self) -> None:
        """Initializes the DSF service by auto-detecting default backend hierarchy.

        Raises:
            RuntimeError: If the backend architecture fails to initialize or connect.

        Example:
            ```python
            service = DSFService()

            ```
        """
        ...

    def query_meta(
        self, id: str, target_backend: Optional[BackendAddr] = None
    ) -> list[ScopedMetaData]:
        """Queries metadata for a specific dataset identifier across backends.

        Args:
            id: The formatted dataset identifier (e.g., "imagenet@v1.0").
            target_backend: Optional specific backend to query.

        Returns:
            A list of `ScopedMetaData` objects matching the identifier.

        Example:
            ```python
            metas = service.query_meta("imagenet@v1.0")
            if metas:
                print("Dataset found.")

            ```
        """
        ...

    def register(
        self,
        name: str,
        tag: str,
        path: str,
        script_path: str,
        owner_nickname: Optional[str] = None,
        dependencies: Optional[list[str]] = None,
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

        Raises:
            RuntimeError: If dataset registration or initialization fails.

        Example:
            ```python
             service.register(
                 name="clean_corpus", tag="v1.0",
                 path="./data/corpus", script_path="./scripts/clean.py",
                 dependencies=["raw_corpus@v1.0"], force_heal=True
             )
            ```
        """
        ...

    def update_merkle(
        self, id: str, target_backend: Optional[BackendAddr] = None
    ) -> None:
        """Recalculates and seals the Merkle tree hash for a registered dataset.

        Essential step before releasing a `BusyStatus.Modifying` concurrency fence.
        It commits the new disk state to storage so subsequent verifications return `Healthy`.

        Args:
            id: The dataset identifier ("name@tag").
            target_backend: Optional target backend where the dataset resides.

        Raises:
            RuntimeError: If recalculating or saving the Merkle tree fails.

        Example:
            ```python
             service.mark_status("corpus@v1", BusyStatus.Modifying)
             # Alter disk files...
             service.update_merkle("corpus@v1")
             service.mark_status("corpus@v1", BusyStatus.Free)

            ```
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
        `BusyStatus.Deleting` before removing physical disk files or unregistering metadata.

        Args:
            id: The dataset identifier to delete.
            force: If True, forces deletion even if downstream datasets depend on this.
            target_backend: Optional target backend to execute deletion against.

        Raises:
            RuntimeError: If deletion fails or if dependencies exist without force=True.

        Example:
            ```python
             service.mark_status("old_data@v0.1", BusyStatus.Deleting)
             import shutil; shutil.rmtree("./data/old_data")  # Remove disk files
             service.delete_metadata("old_data@v0.1", force=True)
            ```
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

        Raises:
            RuntimeError: If the deep verification process fails to execute.

        Example:
            ```python
             res = service.verify_deep("clean_corpus@v1.0")
             if res.status == DatasetStatus.Healthy:
                 print("DAG verified and ready.")

            ```
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
        (such as `BusyStatus.Modifying`), avoiding collisions with ongoing disk IO.

        Args:
            id: The dataset identifier to verify.
            show_diff: If True, logs file-level diffs when corruption is detected.
            target_backend: Optional specific backend to run verification against.

        Returns:
            A `DataSetVerifyRes` containing only the health of target dataset.

        Raises:
            RuntimeError: If self verification process fails to execute.

        Example:
            ```python
             res = service.verify_self("clean_corpus@v1.0")
             if res.status == DatasetStatus.Healthy:
                 print("Disk data is verified and safe for concurrent read.")

            ```
        """
        ...

    def list_all_metadata(self) -> list[ScopedMetaData]:
        """Lists all dataset metadata registered across available backends.

        Returns:
            A list of all discovered `ScopedMetaData` objects.

        Raises:
            IOError: If querying the backend databases fails.
        """
        ...

    def check_is_referenced(self, target_id: str) -> list[ScopedId]:
        """Finds all downstream datasets that depend on the specified dataset.

        Args:
            target_id: The identifier of the parent dataset to check for references.

        Returns:
            A list of `ScopedId` objects representing dependent datasets.

        Raises:
            IOError: If querying the backend graph fails.

        Example:
            ```python
             refs = service.check_is_referenced("raw_corpus@v1.0")
             if refs:
                 print("Cannot modify safely: other datasets depend on this!")

            ```
        """
        ...

    def mark_status(
        self,
        id: str,
        status: BusyStatus,
        target_backend: Optional[BackendAddr] = None,
    ) -> None:
        """Sets or releases a concurrency fence (busy status) on a target dataset.

        Essential for preventing concurrent modification collisions or race conditions
        while mutating disk files or computing Merkle trees.

        Args:
            id: The dataset identifier ("name@tag").
            status: The target concurrency status (e.g., BusyStatus.Modifying or BusyStatus.Free).
            target_backend: Optional specific backend to execute against.

        Raises:
            RuntimeError: If setting the status in the target storage backend fails.

        Example:
            ```python
             service.mark_status("data@v1", BusyStatus.Modifying)
             # Perform disk modifications...
             service.update_merkle("data@v1")
             service.mark_status("data@v1", BusyStatus.Free)

            ```
        """
        ...
