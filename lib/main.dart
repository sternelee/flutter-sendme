import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:file_picker/file_picker.dart';
import 'package:share_plus/share_plus.dart';
import 'package:flutter/services.dart';
import 'src/rust/frb_generated.dart';
import 'src/rust/lib.dart';
import 'src/sendme_provider.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  // initLogging(); // The initLogging function is available but may not be needed
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider(
      create: (context) => SendmeProvider(),
      child: MaterialApp(
        title: 'Sendme - Cross-Platform File Transfer',
        theme: ThemeData(
          colorScheme: ColorScheme.fromSeed(
            seedColor: Colors.blue,
            brightness: Brightness.dark,
          ),
          useMaterial3: true,
        ),
        home: const MainScreen(),
      ),
    );
  }
}

class MainScreen extends StatefulWidget {
  const MainScreen({super.key});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> with TickerProviderStateMixin {
  late TabController _tabController;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Sendme'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        bottom: TabBar(
          controller: _tabController,
          tabs: const [
            Tab(icon: Icon(Icons.upload), text: 'Send'),
            Tab(icon: Icon(Icons.download), text: 'Receive'),
          ],
        ),
      ),
      body: TabBarView(
        controller: _tabController,
        children: const [SendTab(), ReceiveTab()],
      ),
    );
  }
}

class SendTab extends StatefulWidget {
  const SendTab({super.key});

  @override
  State<SendTab> createState() => _SendTabState();
}

class _SendTabState extends State<SendTab> {
  String? _selectedPath;
  bool _isDirectory = false;

  @override
  Widget build(BuildContext context) {
    return Consumer<SendmeProvider>(
      builder: (context, provider, child) {
        return Padding(
          padding: const EdgeInsets.all(16.0),
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Select File or Directory',
                          style: Theme.of(context).textTheme.headlineSmall,
                        ),
                        const SizedBox(height: 16),
                        Row(
                          children: [
                            Expanded(
                              child: ElevatedButton.icon(
                                onPressed: _isSending ? null : _pickFile,
                                icon: const Icon(Icons.file_open),
                                label: const Text('Pick File'),
                              ),
                            ),
                            const SizedBox(width: 8),
                            Expanded(
                              child: ElevatedButton.icon(
                                onPressed: _isSending ? null : _pickDirectory,
                                icon: const Icon(Icons.folder_open),
                                label: const Text('Pick Directory'),
                              ),
                            ),
                          ],
                        ),
                        if (_selectedPath != null) ...[
                          const SizedBox(height: 16),
                          Container(
                            width: double.infinity,
                            padding: const EdgeInsets.all(12),
                            decoration: BoxDecoration(
                              color: Theme.of(
                                context,
                              ).colorScheme.surfaceContainerHighest,
                              borderRadius: BorderRadius.circular(8),
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  _isDirectory ? 'Directory:' : 'File:',
                                  style: Theme.of(
                                    context,
                                  ).textTheme.labelMedium,
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  _selectedPath!,
                                  style: Theme.of(context).textTheme.bodyMedium,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ],
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 16),
                if (provider.isSending)
                  Card(
                    child: Padding(
                      padding: const EdgeInsets.all(16.0),
                      child: Column(
                        children: [
                          Text(
                            '正在发送...',
                            style: Theme.of(context).textTheme.headlineSmall,
                          ),
                          const SizedBox(height: 16),
                          LinearProgressIndicator(value: provider.sendProgress),
                          const SizedBox(height: 8),
                          Text(
                            '${(provider.sendProgress * 100).toInt()}% - ${provider.sendProgressMessage}',
                            style: Theme.of(context).textTheme.bodyMedium,
                          ),
                          if (provider.sendProgress >= 0.8)
                            Text(
                              'Ticket 已生成，等待接收方连接...',
                              style: Theme.of(context).textTheme.bodySmall
                                  ?.copyWith(
                                    color: Colors.orange,
                                    fontWeight: FontWeight.bold,
                                  ),
                            ),
                          Text(
                            '请等待文件处理完成...',
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                  ),
                if (provider.sendResult != null)
                  Card(
                    child: Padding(
                      padding: const EdgeInsets.all(16.0),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Icon(
                                Icons.check_circle,
                                color: Colors.green,
                                size: 28,
                              ),
                              const SizedBox(width: 8),
                              Text(
                                '准备完成，等待接收方连接',
                                style: Theme.of(context).textTheme.headlineSmall
                                    ?.copyWith(color: Colors.green),
                              ),
                            ],
                          ),
                          const SizedBox(height: 8),
                          Container(
                            padding: const EdgeInsets.all(8),
                            decoration: BoxDecoration(
                              color: Colors.orange.withOpacity(0.1),
                              borderRadius: BorderRadius.circular(8),
                              border: Border.all(
                                color: Colors.orange.withOpacity(0.3),
                              ),
                            ),
                            child: Row(
                              children: [
                                Icon(
                                  Icons.info_outline,
                                  color: Colors.orange,
                                  size: 16,
                                ),
                                const SizedBox(width: 4),
                                Expanded(
                                  child: Text(
                                    '文件已准备就绪，数据传输将在接收方连接时自动开始',
                                    style: Theme.of(context).textTheme.bodySmall
                                        ?.copyWith(color: Colors.orange),
                                  ),
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(height: 16),
                          _buildResultInfo(provider.sendResult!),
                          const SizedBox(height: 16),
                          Row(
                            children: [
                              Expanded(
                                child: ElevatedButton.icon(
                                  onPressed: () =>
                                      _copyTicket(provider.sendResult!.ticket),
                                  icon: const Icon(Icons.copy),
                                  label: const Text('复制 Ticket'),
                                ),
                              ),
                              const SizedBox(width: 8),
                              Expanded(
                                child: ElevatedButton.icon(
                                  onPressed: () =>
                                      _shareTicket(provider.sendResult!.ticket),
                                  icon: const Icon(Icons.share),
                                  label: const Text('分享'),
                                  style: ElevatedButton.styleFrom(
                                    backgroundColor: Colors.blue,
                                    foregroundColor: Colors.white,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ),
                if (provider.error != null)
                  Card(
                    color: Theme.of(context).colorScheme.errorContainer,
                    child: Padding(
                      padding: const EdgeInsets.all(16.0),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Error',
                            style: Theme.of(context).textTheme.headlineSmall,
                          ),
                          const SizedBox(height: 8),
                          Text(provider.error!),
                          const SizedBox(height: 8),
                          ElevatedButton(
                            onPressed: provider.clearError,
                            child: const Text('Clear'),
                          ),
                        ],
                      ),
                    ),
                  ),
                const SizedBox(height: 16),
                SizedBox(
                  height: 50,
                  child: ElevatedButton.icon(
                    onPressed: _selectedPath != null && !provider.isSending
                        ? () => _sendFile(_selectedPath!)
                        : null,
                    icon: const Icon(Icons.send),
                    label: const Text('Send File/Directory'),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: Colors.green,
                      foregroundColor: Colors.white,
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  bool get _isSending => context.read<SendmeProvider>().isSending;

  Future<void> _pickFile() async {
    final result = await FilePicker.platform.pickFiles();
    if (result != null && result.files.single.path != null) {
      setState(() {
        _selectedPath = result.files.single.path!;
        _isDirectory = false;
      });
    }
  }

  Future<void> _pickDirectory() async {
    final result = await FilePicker.platform.getDirectoryPath();
    if (result != null) {
      setState(() {
        _selectedPath = result;
        _isDirectory = true;
      });
    }
  }

  Future<void> _sendFile(String path) async {
    final provider = context.read<SendmeProvider>();
    await provider.sendFileToPeer(path);
  }

  Future<void> _copyTicket(String ticket) async {
    await Clipboard.setData(ClipboardData(text: ticket));
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Ticket copied to clipboard!')),
      );
    }
  }

  Future<void> _shareTicket(String ticket) async {
    await Share.share('sendme receive $ticket');
  }

  Widget _buildResultInfo(SendResult result) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildInfoRow('Files:', '${result.fileCount}'),
        _buildInfoRow('Size:', formatBytes(result.size)),
        _buildInfoRow('Hash:', result.hash),
        const SizedBox(height: 8),
        SelectableText(
          'Ticket: ${result.ticket}',
          style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
        ),
      ],
    );
  }

  Widget _buildInfoRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 60,
            child: Text(label, style: Theme.of(context).textTheme.labelMedium),
          ),
          Expanded(
            child: Text(value, style: Theme.of(context).textTheme.bodyMedium),
          ),
        ],
      ),
    );
  }
}

class ReceiveTab extends StatefulWidget {
  const ReceiveTab({super.key});

  @override
  State<ReceiveTab> createState() => _ReceiveTabState();
}

class _ReceiveTabState extends State<ReceiveTab> {
  final _ticketController = TextEditingController();

  @override
  void dispose() {
    _ticketController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<SendmeProvider>(
      builder: (context, provider, child) {
        return Padding(
          padding: const EdgeInsets.all(16.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Card(
                child: Padding(
                  padding: const EdgeInsets.all(16.0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Enter Ticket',
                        style: Theme.of(context).textTheme.headlineSmall,
                      ),
                      const SizedBox(height: 16),
                      TextField(
                        controller: _ticketController,
                        decoration: const InputDecoration(
                          labelText: 'Ticket',
                          hintText: 'Paste the ticket from sender',
                          border: OutlineInputBorder(),
                        ),
                        maxLines: 3,
                        readOnly: provider.isReceiving,
                      ),
                      const SizedBox(height: 16),
                      Row(
                        children: [
                          Expanded(
                            child: ElevatedButton.icon(
                              onPressed: provider.isReceiving
                                  ? null
                                  : _pasteTicket,
                              icon: const Icon(Icons.paste),
                              label: const Text('Paste'),
                            ),
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: ElevatedButton.icon(
                              onPressed:
                                  _ticketController.text.isNotEmpty &&
                                      !provider.isReceiving
                                  ? () => _receiveFile(
                                      _ticketController.text.trim(),
                                    )
                                  : null,
                              icon: const Icon(Icons.download),
                              label: const Text('Receive'),
                              style: ElevatedButton.styleFrom(
                                backgroundColor: Colors.blue,
                                foregroundColor: Colors.white,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 16),
              if (provider.isReceiving)
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      children: [
                        Text(
                          '正在接收...',
                          style: Theme.of(context).textTheme.headlineSmall,
                        ),
                        const SizedBox(height: 16),
                        LinearProgressIndicator(
                          value: provider.receiveProgress,
                        ),
                        const SizedBox(height: 8),
                        Text(
                          '${(provider.receiveProgress * 100).toInt()}% - ${provider.receiveProgressMessage}',
                          style: Theme.of(context).textTheme.bodyMedium,
                        ),
                        Text(
                          '请等待文件下载完成...',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ],
                    ),
                  ),
                ),
              if (provider.receiveResult != null)
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Receive Complete!',
                          style: Theme.of(context).textTheme.headlineSmall,
                        ),
                        const SizedBox(height: 16),
                        _buildReceiveResultInfo(provider.receiveResult!),
                      ],
                    ),
                  ),
                ),
              if (provider.error != null)
                Card(
                  color: Theme.of(context).colorScheme.errorContainer,
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Error',
                          style: Theme.of(context).textTheme.headlineSmall,
                        ),
                        const SizedBox(height: 8),
                        Text(provider.error!),
                        const SizedBox(height: 8),
                        ElevatedButton(
                          onPressed: provider.clearError,
                          child: const Text('Clear'),
                        ),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }

  Future<void> _pasteTicket() async {
    final clipboard = await Clipboard.getData('text/plain');
    if (clipboard?.text != null) {
      setState(() {
        _ticketController.text = clipboard!.text!.trim();
      });
    }
  }

  Future<void> _receiveFile(String ticket) async {
    final provider = context.read<SendmeProvider>();
    await provider.receiveFileFromPeer(ticket);
  }

  Widget _buildReceiveResultInfo(ReceiveResult result) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildInfoRow('Files:', '${result.fileCount}'),
        _buildInfoRow('Size:', formatBytes(result.size)),
        _buildInfoRow('Duration:', '${result.durationMs}ms'),
        _buildInfoRow(
          'Speed:',
          result.durationMs > BigInt.zero
              ? '${formatBytes((result.size * BigInt.from(1000)) ~/ result.durationMs)}/s'
              : 'N/A',
        ),
      ],
    );
  }

  Widget _buildInfoRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 80,
            child: Text(label, style: Theme.of(context).textTheme.labelMedium),
          ),
          Expanded(
            child: Text(value, style: Theme.of(context).textTheme.bodyMedium),
          ),
        ],
      ),
    );
  }
}

String formatBytes(BigInt? bytes) {
  if (bytes == null || bytes <= BigInt.zero) return '0 B';

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  int i = 0;
  double value = bytes.toDouble();

  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }

  return '${value.toStringAsFixed(i == 0 ? 0 : 1)} ${units[i]}';
}
