from unittest.mock import MagicMock, patch

from axon.cli import create_axon_app, docs, run, start_app_normally, start_dev_server


# Unit tests
def test_create_axon_app():
    with patch("axon.cli.prompt") as mock_prompt:
        mock_prompt.return_value = {
            "directory": "test_dir",
            "docker": "N",
            "project_type": "no-db",
        }
        with patch("axon.cli.os.makedirs") as mock_makedirs:
            with patch("axon.cli.shutil.copytree") as mock_copytree, patch("axon.os.remove") as _mock_remove:
                create_axon_app()
                mock_makedirs.assert_called_once()
                mock_copytree.assert_called_once()


def test_docs():
    with patch("axon.cli.webbrowser.open") as mock_open:
        docs()
        mock_open.assert_called_once_with("https://axon.tech")


def test_start_dev_server():
    config = MagicMock()
    config.dev = True
    with patch("axon.cli.setup_reloader") as mock_setup_reloader:
        start_dev_server(config, "test_file.py")
        mock_setup_reloader.assert_called_once()


def test_start_app_normally():
    config = MagicMock()
    config.dev = False
    config.parser.parse_known_args.return_value = (MagicMock(), [])
    with patch("axon.cli.subprocess.run") as mock_run:
        start_app_normally(config)
        mock_run.assert_called_once()


# Integration tests
def test_run_create():
    with patch("axon.cli.Config") as mock_config:
        mock_config.return_value.create = True
        with patch("axon.cli.create_axon_app") as mock_create:
            run()
            mock_create.assert_called_once()
